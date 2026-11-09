"""Offline wrist-cut hand cycle selection, not a general tube detector.
Usage: make-hand-cycle-paths.py raw-snapshot.json output-directory
Uses geodesic distance from the known wrist plane, extracts source-edge cycles,
and records their transported integer periods before selecting one per branch.
"""
from pathlib import Path
import sys,json,numpy as np,heapq,math
input_path=Path(sys.argv[1]);out=Path(sys.argv[2]);out.mkdir(parents=True,exist_ok=True);d=json.loads(input_path.read_text());f=d['field'];p=np.array(f['surface']['positions']);t=np.array(f['surface']['triangles']);q=np.array(f['directions']);n=np.array(f['normals']);edges=d['lattice']['edges'];lookup={tuple(e['ends']):i for i,e in enumerate(edges)};adj=[{} for _ in p]
for e in edges:
 a,b=e['ends'];adj[a][b]=adj[b][a]=np.linalg.norm(p[a]-p[b])
dist=np.full(len(p),np.inf);seed=np.where(abs(p[:,0]+p[:,2]-3.2)<1e-9)[0];dist[seed]=0;todo=[(0.,int(i)) for i in seed];heapq.heapify(todo)
while todo:
 x,i=heapq.heappop(todo)
 if x>dist[i]:continue
 for j,length in adj[i].items():
  z=x+length
  if z<dist[j]:dist[j]=z;heapq.heappush(todo,(z,j))
def rot(v,r):
 x,y=v;return [(x,y),(-y,x),(-x,-y),(y,-x)][r%4]
def analyze(path):
 r=0;period=np.zeros(2,dtype=int);scores=np.zeros(2);entries=[]
 for u,v in zip(path,path[1:]+path[:1]):
  eid=lookup[tuple(sorted((u,v)))];e=edges[eid];forward=u==e['ends'][0];ra=r if forward else r-e['transport'];delta=p[v]-p[u];tangent=delta-n[u]*(delta@n[u]);tangent/=max(np.linalg.norm(tangent),1e-12)
  dirs=[q[u],np.cross(n[u],q[u])]
  for a in range(2):scores[a]+=abs(dirs[(a-r)%2]@tangent)*np.linalg.norm(delta)
  period+=np.array(rot(e['d'],ra))*(1 if forward else -1);entries.append((eid,ra));r+=e['transport']*(1 if forward else -1)
 along=int(np.argmax(scores));return {'rotation':int(r%4),'period':period.tolist(),'along_axis':along,'axial_period':int(period[1-along]),'circumferential_period':int(period[along]),'mean_alignment':float(max(scores)/sum(np.linalg.norm(p[v]-p[u]) for u,v in zip(path,path[1:]+path[:1])))}
h=f['spacing'][0];cycles=[]
for level in np.arange(h,dist.max(),h*2):
 inc={}
 for face in t:
  if np.min(dist[face])<=level:continue
  for a,b in zip(face,np.roll(face,-1)):
   key=tuple(sorted((int(a),int(b))));inc.setdefault(key,[]).append((int(a),int(b)))
 boundary=[v[0] for v in inc.values() if len(v)==1];nxt={a:b for a,b in boundary}
 if len(nxt)!=len(boundary):continue
 used=set()
 for start in nxt:
  if start in used:continue
  path=[start];cur=nxt[start]
  while cur not in path and cur in nxt:path.append(cur);cur=nxt[cur]
  used.update(path)
  if cur!=start:continue
  xyz=p[path];perim=float(np.linalg.norm(np.roll(xyz,-1,axis=0)-xyz,axis=1).sum())
  if xyz[:,2].min()<1.18 or perim>.95 or perim<6*h:continue
  info=analyze(path)
  if info['rotation']!=0:continue
  center=xyz.mean(0);finger='short_outer' if center[0]>2.66 else 'adjacent_long' if center[0]>2.52 else 'central_long' if center[0]>2.3 else 'opposite_outer'
  cycles.append({'finger':finger,'level':float(level),'center':center.tolist(),'perimeter':perim,'vertices':path,**info})
selected=[]
for finger in sorted(set(c['finger'] for c in cycles)):
 candidates=[c for c in cycles if c['finger']==finger and abs(c['circumferential_period'])>=4]
 if candidates:selected.append(max(candidates,key=lambda c:c['perimeter']*c['mean_alignment']))
(out/'lattice-cycle-analysis.json').write_text(json.dumps({'candidates':cycles,'selected':selected},indent=2)+'\n');(out/'lattice-cycles.json').write_text(json.dumps([c['vertices'] for c in selected])+'\n');print('selected',[(c['finger'],len(c['vertices']),c['period'],c['along_axis'],c['mean_alignment']) for c in selected]);print('all',[(c['finger'],c['period']) for c in cycles])
