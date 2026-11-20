"""10 階・100 階の建物に各階 100 枚の壁を置く、計測用の自作 IFC4。"""

import argparse
from pathlib import Path


def root(number, kind, attributes):
    return f"#{number}={kind}('{number:022}',{attributes});"


def building(floors):
    lines = [
        "ISO-10303-21;",
        "HEADER;",
        "FILE_DESCRIPTION(('ViewDefinition [CoordinationView]'),'2;1');",
        "FILE_NAME('live.ifc','2026-11-21T00:00:00',(),(),'zumen verification','zumen','');",
        "FILE_SCHEMA(('IFC4'));",
        "ENDSEC;",
        "DATA;",
        root(1, "IFCPROJECT", "$,'Project',$,$,$,$,(#7),#5"),
        "#2=IFCCARTESIANPOINT((0.,0.,0.));",
        "#3=IFCAXIS2PLACEMENT3D(#2,$,$);",
        "#4=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);",
        "#5=IFCUNITASSIGNMENT((#4));",
        "#6=IFCLOCALPLACEMENT($,#3);",
        "#7=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,0.00001,#3,$);",
        root(8, "IFCBUILDING", "$,'Building',$,$,#6,$,$,.ELEMENT.,$,$,$"),
        root(9, "IFCRELAGGREGATES", "$,$,$,#1,(#8)"),
        "#10=IFCRECTANGLEPROFILEDEF(.AREA.,$,$,1.,0.2);",
        "#11=IFCDIRECTION((0.,0.,1.));",
        "#12=IFCEXTRUDEDAREASOLID(#10,#3,#11,3.);",
        "#13=IFCSHAPEREPRESENTATION(#7,'Body','SweptSolid',(#12));",
    ]
    storeys = []
    for floor in range(floors):
        storey = 1000 + floor * 1000
        storeys.append(f"#{storey}")
        lines.extend([
            f"#{storey+1}=IFCCARTESIANPOINT((0.,0.,{floor*3}.));",
            f"#{storey+2}=IFCAXIS2PLACEMENT3D(#{storey+1},$,$);",
            f"#{storey+3}=IFCLOCALPLACEMENT(#6,#{storey+2});",
            root(storey, "IFCBUILDINGSTOREY", f"$,'Storey {floor}',$,$,#{storey+3},$,$,.ELEMENT.,{floor*3}."),
        ])
        walls = []
        for index in range(100):
            wall = storey + 10 + index * 6
            walls.append(f"#{wall}")
            x, y = (index % 10) * 4, (index // 10) * 4
            lines.extend([
                f"#{wall+1}=IFCCARTESIANPOINT(({x}.,{y}.,0.));",
                f"#{wall+2}=IFCAXIS2PLACEMENT3D(#{wall+1},$,$);",
                f"#{wall+3}=IFCLOCALPLACEMENT(#{storey+3},#{wall+2});",
                f"#{wall+4}=IFCPRODUCTDEFINITIONSHAPE($,$,(#13));",
                root(wall, "IFCWALL", f"$,'Wall',$,$,#{wall+3},#{wall+4},$,.STANDARD."),
            ])
        lines.append(root(storey + 4, "IFCRELCONTAINEDINSPATIALSTRUCTURE",
                          f"$,$,$,({','.join(walls)}),#{storey}"))
    lines.append(root(20, "IFCRELAGGREGATES", f"$,$,$,#8,({','.join(storeys)})"))
    lines.extend(["ENDSEC;", "END-ISO-10303-21;", ""])
    return "\n".join(lines).encode("ascii")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    for floors in (10, 100):
        (args.output / f"{floors*100}-wall.ifc").write_bytes(building(floors))
