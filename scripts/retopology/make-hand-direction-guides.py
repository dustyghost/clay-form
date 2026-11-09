"""Offline curvature-direction experiment; NumPy only, no runtime dependency.
Usage: script source.clay output-directory
The selected finger region and neighborhood scale are experiment settings,
not a general-purpose tube detector. The cap and palm are left unconstrained.
"""
import sys,json,math,heapq
from pathlib import Path
import numpy as np

RADIUS=.085
GUIDE_RADIUS=.065
CELL=.03

def estimate(vertices, triangles):
    p=np.asarray(vertices,dtype=float);t=np.asarray(triangles,dtype=int)
    cross=np.cross(p[t[:,1]]-p[t[:,0]],p[t[:,2]]-p[t[:,0]])
    n=np.zeros_like(p);areas=np.zeros(len(p));adj=[set() for _ in p]
    for tri,c in zip(t,cross):
        for a,b in zip(tri,np.roll(tri,-1)):
            adj[a].add(int(b));adj[b].add(int(a));n[a]+=c;areas[a]+=np.linalg.norm(c)/6
    n/=np.maximum(np.linalg.norm(n,axis=1)[:,None],1e-30)
    candidates=[]
    for i,origin in enumerate(p):
        if origin[2]<=1.15 or origin[0]+origin[2]<3.36:continue
        # Connected geodesic neighborhood avoids jumping to a nearby finger.
        distances={i:0.};queue=[(0.,i)]
        while queue:
            d,j=heapq.heappop(queue)
            if d>distances[j]:continue
            for k in adj[j]:
                nd=d+np.linalg.norm(p[k]-p[j])
                if nd<RADIUS and nd<distances.get(k,float('inf')):
                    distances[k]=nd;heapq.heappush(queue,(nd,k))
        ids=np.array([j for j in distances if j!=i and n[j]@n[i]>.5],dtype=int)
        if len(ids)<12:continue
        axis=np.array([1.,0.,0.]) if abs(n[i,0])<.8 else np.array([0.,1.,0.])
        u=np.cross(n[i],axis);u/=np.linalg.norm(u);v=np.cross(n[i],u)
        delta=p[ids]-origin;dn=n[ids]-n[i];x=delta@u;y=delta@v
        a=np.zeros((len(ids)*2,3));a[0::2,0]=x;a[0::2,1]=y;a[1::2,1]=x;a[1::2,2]=y
        b=np.stack((dn@u,dn@v),axis=1).reshape(-1)
        w=np.sqrt(areas[ids])*np.maximum(.05,1-np.array([distances[j] for j in ids])/RADIUS)
        w=np.repeat(w,2);a*=w[:,None];b*=w
        fit=np.linalg.lstsq(a,b,rcond=None)[0]
        residual=np.linalg.norm(a@fit-b)/max(np.linalg.norm(b),1e-12)
        vals,vecs=np.linalg.eigh([[fit[0],fit[1]],[fit[1],fit[2]]]);order=np.argsort(abs(vals));small,large=abs(vals[order]);confidence=1-small/max(large,1e-12)
        if large<2 or confidence<.65 or residual>.65:continue
        q=vecs[:,order[0]];direction=q[0]*u+q[1]*v
        candidates.append({'vertex':i,'position':origin.tolist(),'normal':n[i].tolist(),'tangent':direction.tolist(),'radius':GUIDE_RADIUS,'strength':float(.7*confidence),'curvatures':vals.tolist(),'confidence':float(confidence),'relative_fit_residual':float(residual),'neighborhood_vertices':len(ids)})
    bins={}
    for g in candidates:
        key=tuple(np.floor(np.asarray(g['position'])/CELL).astype(int))
        if key not in bins or g['confidence']>bins[key]['confidence']:bins[key]=g
    return p,n,sorted(bins.values(),key=lambda g:g['vertex'])

if __name__=='__main__':
    source=Path(sys.argv[1]);out=Path(sys.argv[2]);out.mkdir(parents=True,exist_ok=True)
    mesh=json.loads(source.read_text())['mesh'];p,n,samples=estimate(mesh['vertices'],mesh['triangles'])
    fields=['position','normal','tangent','radius','strength']
    with (out/'guides.json').open('x') as f:json.dump([{k:g[k] for k in fields} for g in samples],f)
    with (out/'guide-analysis.json').open('x') as f:json.dump({'method':'Area-weighted local symmetric normal-derivative fit in connected geodesic neighborhoods; use least absolute curvature direction where anisotropy is strong. Sparse voxel thinning. Explicit finger-region restriction; existing guides also softly influence position phase.','neighborhood_radius':RADIUS,'guide_radius':GUIDE_RADIUS,'thinning_cell':CELL,'region':'z > 1.15 and x + z >= 3.36','guides':samples},f)
    print('guide count',len(samples))
