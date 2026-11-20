# Calternal iPhone shell

A thin native Swift shell around the web app. Everything about it (layout, start address,
the `window.calternalNative` bridge, what the page must do, the Simulator workflow) is in
[docs/ios-shell.md](../../docs/ios-shell.md).

Quick start (on the Mac Studio, in this directory): copy `Local.xcconfig.example` to
`Local.xcconfig`, then `./sim.sh build && ./sim.sh run`.
