#!/usr/bin/env python3
"""Generate a pinned QuadriFlow dynamic-position reference executable.

Usage: make-dynamic-reference.py upstream output.cpp [normalize-axis] [trace]
The optional variant fixes upstream's discarded axis.normalized() return value;
The trace variant records iterations and linear-solver status through QF_TRACE.
These are explicit harness changes; the upstream checkout is never modified.
"""
import pathlib
import subprocess
import sys
PIN = "810b7a0967c35b0dc85b4464e3835e26a756c967"
root = pathlib.Path(sys.argv[1]).resolve()
out = pathlib.Path(sys.argv[2])
for relative in ("src/optimizer.cpp", "LICENSE.txt"):
    committed = subprocess.check_output(["git", "-C", str(root), "show", f"{PIN}:{relative}"])
    if committed != (root / relative).read_bytes():
        raise SystemExit(f"Modified reference source: {relative}")
source = (root / "src/optimizer.cpp").read_text()
function = source[source.index("void Optimizer::optimize_positions_dynamic("):source.index("void Optimizer::optimize_positions_sharp(")]
function = function.replace("void Optimizer::optimize_positions_dynamic(", "void optimize_positions_dynamic(", 1)
variants = set(sys.argv[3:])
if variants - {"normalize-axis", "trace"}:
    raise SystemExit("Unknown reference variant")
if "normalize-axis" in variants:
    function = function.replace("axis.normalized();", "axis.normalize();", 1)
if "trace" in variants:
    function = function.replace("    std::set<int> uncertain;", '    std::ofstream trace(std::getenv("QF_TRACE"));\n    trace << std::setprecision(17);\n    std::set<int> uncertain;', 1)
    function = function.replace("        // forgive my hack...", r'''        trace << "ITER " << iter << " " << O_compact.size() << "\n";
        for (int i=0;i<O_compact.size();++i) trace << Vind[i] << ' ' << O_compact[i][0] << ' ' << O_compact[i][1] << ' ' << O_compact[i][2] << '\n';
        // forgive my hack...''', 1)
    function = function.replace("        VectorXd x_new = solver.solve(rhs);", '        VectorXd x_new = solver.solve(rhs);\n        std::cerr << "ITER " << iter << " solver-info " << solver.info() << " relative-residual " << (A*x_new-rhs).norm()/rhs.norm() << std::endl;', 1)
header = r'''
#include <Eigen/Core>
#include <Eigen/Geometry>
#include <Eigen/Sparse>
#include <iostream>
#include <fstream>
#include <cstdlib>
#include <iomanip>
#include <map>
#include <set>
#include <list>
#include <unordered_map>
#include <vector>
using namespace Eigen;
template<class T> using LinearSolver = Eigen::SimplicialLLT<T>;
'''
main = r'''
int main() {
    int n,f,v,q;
    if (!(std::cin >> n >> f >> v >> q)) return 2;
    MatrixXd V(3,n),N(3,n),Q(3,n);
    MatrixXi F(3,f);
    for (int i=0;i<n;++i) for (auto p : {&V,&N,&Q}) for (int k=0;k<3;++k) std::cin >> (*p)(k,i);
    for (int i=0;i<f;++i) for (int k=0;k<3;++k) std::cin >> F(k,i);
    std::vector<Vector3d> O(v);
    std::vector<std::vector<int>> Vset(v);
    for (int i=0;i<v;++i) {
        for (int k=0;k<3;++k) std::cin >> O[i][k];
        int s;std::cin >> s;Vset[i].resize(s);
        for (int &x:Vset[i]) std::cin >> x;
    }
    std::vector<Vector4i> faces(q);
    std::vector<int> V2E(v,-1),E2E(q*4,-1),counts(q*4),sharp(v,0);
    std::map<std::pair<int,int>,int> o2e;
    for (int i=0;i<q;++i) {
        for (int k=0;k<4;++k) std::cin >> faces[i][k];
        for (int k=0;k<4;++k) {
            int a=faces[i][k], b=faces[i][(k+1)%4];
            if (V2E[a]==-1) V2E[a]=i*4+k;
            o2e[{a,b}]=i*4+k;
        }
    }
    for (auto info:o2e) {
        auto it=o2e.find({info.first.second,info.first.first});
        if (it!=o2e.end()) E2E[info.second]=it->second;
    }
    std::vector<Vector3d> diffs(q*4);
    for (int i=0;i<q*4;++i) {
        for (int k=0;k<3;++k) std::cin >> diffs[i][k];
        std::cin >> counts[i];
    }
    if (!std::cin) return 3;
    std::map<int,std::pair<Vector3d,Vector3d>> constraints;
    optimize_positions_dynamic(F,V,N,Q,Vset,O,faces,V2E,E2E,1.,diffs,counts,o2e,sharp,constraints,1);
    std::cout << std::setprecision(17);
    for (const auto &p:O) std::cout << p[0] << ' ' << p[1] << ' ' << p[2] << '\n';
}
'''
license_text = (root / "LICENSE.txt").read_text()
with out.open("x") as stream:
    stream.write(f"/* QuadriFlow {PIN}\n{license_text}\n*/\n" + header + function + main)
