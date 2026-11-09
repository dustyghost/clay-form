"""Render a rejected or accepted patch candidate beside the unmodified sculpt."""
import argparse,json,math
from pathlib import Path
import numpy as np
from PIL import Image,ImageDraw,ImageFont
parser=argparse.ArgumentParser()
parser.add_argument('source',type=Path);parser.add_argument('candidate',type=Path);parser.add_argument('quality',type=Path);parser.add_argument('output',type=Path)
parser.add_argument('--two-sided',action='store_true',help='Show camera-facing backs in magenta; diagnostic only.')
parser.add_argument('--no-wire',action='store_true',help='Inspect surface coverage without dense edge lines.')
parser.add_argument('--zoom',type=float,default=1.)
parser.add_argument('--center',type=float,nargs=3,help='World-space camera target; only changes the view.')
parser.add_argument('--yaw',type=float,default=.32)
parser.add_argument('--pitch',type=float,default=-.18)
parser.add_argument('--candidate-label',help='Display label for other diagnostic comparisons.')
parser.add_argument('--caption',help='Explicit caption, including diagnostic status.')
parser.add_argument('--extra',nargs=3,action='append',default=[],metavar=('MESH','QUALITY','LABEL'),help='Additional saved candidate panel.')
args=parser.parse_args()
if args.zoom<=0 or not math.isfinite(args.zoom):parser.error('zoom must be positive and finite')
source=json.loads(args.source.read_text())['mesh'];data=json.loads(args.candidate.read_text());quality=json.loads(args.quality.read_text())
source['quads']=[]
candidate={'vertices':data['positions'],'quads':data['quads'],'triangles':data.get('triangles',[t for x,y,z,w in data['quads'] for t in ([x,y,z],[x,z,w])])}
inputs=[('Original sculpt',None),(args.candidate_label or f"Quad patch candidate: {len(data['quads']):,} quads",None)]
meshes=[source,candidate];bad_sets=[set(),{x['quad'] for x in quality['bad_quads']}]
for mesh_path,quality_path,label in args.extra:
 extra=json.loads(Path(mesh_path).read_text());eq=json.loads(Path(quality_path).read_text())
 meshes.append({'vertices':extra['positions'],'quads':extra['quads'],'triangles':[t for x,y,z,w in extra['quads'] for t in ([x,y,z],[x,z,w])]})
 inputs.append((label,None));bad_sets.append({x['quad'] for x in eq['bad_quads']})
yaw,pitch=args.yaw,args.pitch
s,c=math.sin(yaw),math.cos(yaw); p,q=math.sin(pitch),math.cos(pitch)
right=np.array([c,0,-s]);up=np.array([s*p,q,c*p]);view=np.array([s*q,-p,c*q]);basis=np.stack((right,-up,view),axis=1)
W,H=640,850
v0=np.asarray(meshes[0]['vertices'])@basis
lo=v0[:,:2].min(axis=0);hi=v0[:,:2].max(axis=0);scale=min((W-64)/(hi[0]-lo[0]),(H-100)/(hi[1]-lo[1]));center=(lo+hi)/2
scale*=args.zoom
if args.center is not None:center=(np.asarray(args.center)@basis)[:2]
font=ImageFont.truetype('/usr/share/fonts/gsfonts/NimbusSans-Regular.otf',20)
canvas=Image.new('RGB',(W*len(meshes),H+45),(24,28,32))
for panel,((label,_),mesh) in enumerate(zip(inputs,meshes)):
 bad=bad_sets[panel]
 xyz=np.asarray(mesh['vertices'],dtype=np.float64); pr=xyz@basis
 xy=(pr[:,:2]-center)*scale+np.array([W/2,(H+36)/2]);depth=pr[:,2]
 pix=np.full((H,W,3),[24,28,32],dtype=np.uint8); zbuf=np.full((H,W),-np.inf)
 triangles=np.asarray(mesh['triangles'],dtype=np.int32)
 normals=np.cross(xyz[triangles[:,1]]-xyz[triangles[:,0]],xyz[triangles[:,2]]-xyz[triangles[:,0]])
 normals/=np.maximum(np.linalg.norm(normals,axis=1)[:,None],1e-30)
 light=view+up*.4-right*.3;light/=np.linalg.norm(light)
 for ti,(t,n) in enumerate(zip(triangles,normals)):
  back=n@view<=0
  if back and not args.two_sided: continue
  a,b,c=xy[t];xmin=max(0,int(np.floor(min(a[0],b[0],c[0]))));xmax=min(W-1,int(np.ceil(max(a[0],b[0],c[0]))));ymin=max(50,int(np.floor(min(a[1],b[1],c[1]))));ymax=min(H-1,int(np.ceil(max(a[1],b[1],c[1]))))
  if xmin>xmax or ymin>ymax:continue
  yy,xx=np.mgrid[ymin:ymax+1,xmin:xmax+1];xx=xx+.5;yy=yy+.5
  den=(b[1]-c[1])*(a[0]-c[0])+(c[0]-b[0])*(a[1]-c[1])
  if abs(den)<1e-12:continue
  u=((b[1]-c[1])*(xx-c[0])+(c[0]-b[0])*(yy-c[1]))/den
  vv=((c[1]-a[1])*(xx-c[0])+(a[0]-c[0])*(yy-c[1]))/den
  zz=u*depth[t[0]]+vv*depth[t[1]]+(1-u-vv)*depth[t[2]]
  section=zbuf[ymin:ymax+1,xmin:xmax+1];ok=(u>=-1e-7)&(vv>=-1e-7)&(u+vv<=1.0000001)&(zz>section)
  section[ok]=zz[ok]
  shade=.35+.65*max(0,float((-n if back else n)@light));pix[ymin:ymax+1,xmin:xmax+1][ok]=np.array([238,90,227] if back else [225,104,86] if panel and ti//2 in bad else [117,184,181])*shade
 edges=set()
 for a,b,c in triangles:
  edges.update((tuple(sorted((int(a),int(b)))),tuple(sorted((int(b),int(c)))),tuple(sorted((int(c),int(a))))))
 for a,b,c,d in mesh['quads']:
  edges.discard(tuple(sorted((a,c))));edges.discard(tuple(sorted((b,d))))
 for a,b in (edges if panel and not args.no_wire else []):
  n=max(2,int(np.max(np.abs(xy[b]-xy[a]))*1.5)+1);f=np.linspace(0,1,n)
  coords=xy[a]+f[:,None]*(xy[b]-xy[a]);ix=np.floor(coords[:,0]).astype(int);iy=np.floor(coords[:,1]).astype(int);zz=depth[a]+f*(depth[b]-depth[a])
  valid=(ix>=0)&(ix<W)&(iy>=50)&(iy<H);ix=ix[valid];iy=iy[valid];zz=zz[valid]
  visible=zz>=zbuf[iy,ix]-.008
  pix[iy[visible],ix[visible]]=[25,42,46]
 im=Image.fromarray(pix);ImageDraw.Draw(im).text((20,16),label,font=font,fill=(233,238,240));canvas.paste(im,(panel*W,0))
caption=f"{'Accepted by current checks' if data['accepted'] else 'REJECTED candidate'} | {quality['quads_without_outward_diagonal']} quads fail outward checks (red). Fixed render diagonals."
if args.two_sided:caption+=' Camera-facing backs: magenta.'
if args.caption:caption=args.caption
ImageDraw.Draw(canvas).text((18,H+10),caption,font=font,fill=(245,197,152))
canvas.save(args.output)
print(args.output)
