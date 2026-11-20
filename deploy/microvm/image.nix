# Build with the repository's locked nixpkgs. The normalized OCI root and static
# executables are input store paths so the builder has no network dependency.
{ pkgs, agent, cli, standardRoot }:
let
  runtime = with pkgs; [ busybox podman conmon crun catatonit socat shadow coreutils slirp4netns ];
  closure = pkgs.closureInfo { rootPaths = runtime; };
  workerLauncher = pkgs.pkgsStatic.stdenv.mkDerivation {
    name = "calternal-worker-launch";
    dontUnpack = true;
    buildPhase = ''
      $CC -O2 -Wall -Wextra -Werror ${./worker-launch.c} -o worker-launch
    '';
    installPhase = "install -Dm0755 worker-launch $out/bin/worker-launch";
  };
  muslCompiler = pkgs.pkgsCross.musl64.stdenv.cc;
  nodeHeaders = pkgs.lib.getDev pkgs.nodejs_24;
  reseedAddon = pkgs.runCommand "calternal-reseed.node" {
    nativeBuildInputs = [ muslCompiler pkgs.binutils ];
    CC_x86_64_unknown_linux_musl = "${muslCompiler}/bin/${muslCompiler.targetPrefix}cc";
    CALTERNAL_NODE_HEADERS = "${nodeHeaders}/include/node";
  } ''
    bash ${./build-addon.sh} ${./reseed-addon.c} "$out"
  '';
  seccomp = pkgs.runCommand "calternal-worker-bpf" {
    nativeBuildInputs = [ pkgs.python3 ];
    LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [ pkgs.libseccomp ];
  } ''
    python3 ${./worker-seccomp.py} ${../../crates/machine/src/sandbox/worker-seccomp.json} "$out"
  '';
  kernel = pkgs.fetchurl {
    url = "https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/20261111-778a26c0594c-0/x86_64/vmlinux-6.1.188";
    hash = "sha256-9XNXdOJXDZ30UfxWZfNSXTuvcj+mqKQToDWR1XQrOaU=";
  };
in
pkgs.runCommand "calternal-microvm-x86_64-v2" {
  nativeBuildInputs = with pkgs; [ fakeroot e2fsprogs ];
  SOURCE_DATE_EPOCH = "1735689600";
  E2FSPROGS_FAKE_TIME = "1735689600";
} ''
  mkdir -p "$out" root/{bin,sbin,usr/bin,dev,proc,sys,tmp,run,root,etc/containers,data,opt,var/tmp,nix/store}
  while read -r storePath; do
    cp -a "$storePath" "root$storePath"
  done < "${closure}/store-paths"
  for applet in $(${pkgs.busybox}/bin/busybox --list); do
    ln -s ${pkgs.busybox}/bin/busybox "root/bin/$applet"
  done
  for program in podman socat slirp4netns; do
    case "$program" in
      podman) programPath=${pkgs.podman}/bin/podman ;;
      slirp4netns) programPath=${pkgs.slirp4netns}/bin/slirp4netns ;;
      socat) programPath=${pkgs.socat}/bin/socat ;;
    esac
    ln -s "$programPath" "root/usr/bin/$program"
  done
  ln -s ${pkgs.coreutils}/bin/sync root/usr/bin/sync
  install -m 0755 ${pkgs.shadow}/bin/newuidmap root/usr/bin/newuidmap
  install -m 0755 ${pkgs.shadow}/bin/newgidmap root/usr/bin/newgidmap
  install -m 0755 ${./init.sh} root/init
  install -m 0755 ${./agent-start.sh} root/sbin/agent-start
  install -m 0755 ${agent} root/opt/calternal-microvm-agent
  install -m 0755 ${cli} root/opt/calternal
  mkdir root/opt/standard
  tar --no-same-owner -xzf ${standardRoot} -C root/opt/standard
  mkdir -p root/opt/standard/{plugin,proc,dev,opt/calternal-standby}
  install -m 0444 ${./standby.cjs} root/opt/standard/opt/calternal-standby/standby.cjs
  for addon in reseed.node reseed-second.node; do
    install -m 0444 ${reseedAddon} "root/opt/standard/opt/calternal-standby/$addon"
  done
  (cd root/opt/standard/opt/calternal-standby; sha256sum reseed.node) > "$out/reseed.sha256"
  install -m 0755 ${workerLauncher}/bin/worker-launch root/opt/calternal-worker-launch
  install -m 0444 ${seccomp} root/opt/calternal-worker.bpf

  printf 'root:x:0:0:root:/root:/bin/sh\nagent:x:1000:1000:agent:/data/home:/bin/sh\n' > root/etc/passwd
  printf 'root:x:0:\nagent:x:1000:\n' > root/etc/group
  printf 'agent:100000:65536\n' > root/etc/subuid
  cp root/etc/subuid root/etc/subgid
  ln -s /run/calternal-resolv.conf root/etc/resolv.conf
  printf '127.0.0.1 localhost\n' > root/etc/hosts
  printf '{"default":[{"type":"reject"}]}\n' > root/etc/containers/policy.json
  cat > root/etc/containers/containers.conf <<CONF
  [engine]
  cgroup_manager="cgroupfs"
  events_logger="file"
  runtime="${pkgs.crun}/bin/crun"
  conmon_path=["${pkgs.conmon}/bin/conmon"]
  init_path="${pkgs.catatonit}/bin/catatonit"
  CONF
  find root -print0 | xargs -0 touch -h -d "@$SOURCE_DATE_EPOCH"
  cp ${kernel} "$out/vmlinux"
  size=$(du -s --block-size=1M root | cut -f1)
  truncate -s "$((size + 128))M" "$out/root.ext4"
  fakeroot -- sh -ec '
    chown -hR 0:0 root
    chown -hR 1000:1000 root/opt/standard
    mkfs.ext4 -q -F -O ^has_journal -m 0 -U f792e824-b9c6-4a01-b079-1fe1ad7435b4 \
      -E root_owner=0:0,lazy_itable_init=0,hash_seed=134976e2-fc97-4129-9d89-331910dbbaa2 \
      -d root "$1"
  ' sh "$out/root.ext4"
  # Set privilege bits in the guest filesystem through its inode records.
  # The Nix build never creates setuid executables in the host build tree.
  for helper in newuidmap newgidmap; do
    debugfs -w -R "set_inode_field /usr/bin/$helper mode 0104755" "$out/root.ext4"
  done
  debugfs -w -R "set_inode_field /opt/calternal-worker-launch mode 0104755" "$out/root.ext4"
  (cd "$out"; sha256sum vmlinux root.ext4 > image.sha256)
''
