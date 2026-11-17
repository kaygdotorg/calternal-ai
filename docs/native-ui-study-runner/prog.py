import re,sys
def load(p='/tmp/r/diff.txt'):
    t=None; rows=[]
    for l in open(p):
        m=re.search(r'pts_time:([\d.]+)',l)
        if m: t=float(m.group(1))
        m=re.search(r'YAVG=([\d.]+)',l)
        if m: rows.append((t,float(m.group(1))))
    return rows
def event(rows,t0,span=1.0,floor=0.25):
    seg=[(t,v) for t,v in rows if t0-0.02<=t<=t0+span]
    # trim trailing tail below floor
    tot=sum(v for _,v in seg)
    cum=0; out={}
    for t,v in seg:
        cum+=v
        for q in (0.1,0.5,0.9,0.98):
            if q not in out and cum>=q*tot: out[q]=round(t-seg[0][0],3)
    return out
if __name__=="__main__":
    rows=load()
    for t0 in map(float,sys.argv[1:]): print(t0,event(rows,t0))
