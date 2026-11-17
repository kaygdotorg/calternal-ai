import re,sys,subprocess
f=sys.argv[1]; vf=sys.argv[2]; thr=float(sys.argv[3]) if len(sys.argv)>3 else 0.05
subprocess.run(["ffmpeg","-v","error","-y","-i",f,"-vf",vf+",format=gray,tblend=all_mode=difference,signalstats,metadata=print:key=lavfi.signalstats.YAVG:file=/tmp/r/diff.txt","-f","null","-"])
t=None; rows=[]
for l in open('/tmp/r/diff.txt'):
    m=re.search(r'pts_time:([\d.]+)',l)
    if m: t=float(m.group(1))
    m=re.search(r'YAVG=([\d.]+)',l)
    if m: rows.append((t,float(m.group(1))))
print(len(rows),'frames; median dt',sorted(rows[i+1][0]-rows[i][0] for i in range(len(rows)-1))[len(rows)//2])
bursts=[];cur=[]
for t,v in rows:
    if v>thr:
        if cur and t-cur[-1][0]>0.12: bursts.append(cur);cur=[]
        cur.append((t,v))
if cur: bursts.append(cur)
for b in bursts: print('t0=%.3f dur=%.3f n=%d peak=%.1f'%(b[0][0],b[-1][0]-b[0][0],len(b),max(v for _,v in b)),' '.join('%.1f'%v for _,v in b)[:150])
