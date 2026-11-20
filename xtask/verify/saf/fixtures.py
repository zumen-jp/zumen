"""#494 の自作 SFC・P21・SAF。仕様本文や製品の付属資源を転載しない。"""


def sfc(saf_name):
    features = [
        r"layer_feature(\'L\','1')",
        r"pre_defined_font_feature(\'continuous\')",
        r"pre_defined_colour_feature(\'black\')",
        "width_feature('0.13')",
        "line_feature('1','1','1','1','0.0','0.0','100.0','0.0')",
        f"sfig_org_feature(\\'$$ATRF$$10$${saf_name}\\','3')",
        f"sfig_locate_feature('1',\\'$$ATRF$$10$${saf_name}\\','0','0','0','1','1')",
        "circle_feature('1','1','1','1','50.0','25.0','10.0')",
        r"drawing_sheet_feature(\'S\','3','1','420','297')",
    ]
    header = """ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('SCADEC level2 feature_mode'),'2;1');
FILE_NAME('DRAWING.SFC','2026-11-16T00:00:00',(''),(''),'zumen$$3.1','zumen','');
FILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));
ENDSEC;
DATA;
"""
    records = "".join(f"/*SXF\n#{index * 10} = {feature}\nSXF*/\n" for index, feature in enumerate(features, 1))
    return (header + records + "ENDSEC;\nEND-ISO-10303-21;\n").encode("ascii")


def p21(saf_name):
    # OCF「SXF仕様について」の P21 の用紙例。図面版と用紙版を結び、用紙寸法は同じ枠を指す。
    # https://ocf.or.jp/pdf/2001kickoff2.pdf
    return f"""ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('SCADEC level2 AP202_mode'),'2;1');
FILE_NAME('DRAWING.P21','2026-11-16T00:00:00',(''),(''),'zumen$$3.1','zumen','');
FILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));
ENDSEC;
DATA;
#1=DRAUGHTING_PRE_DEFINED_COLOUR('black');
#2=DRAUGHTING_PRE_DEFINED_CURVE_FONT('continuous');
#3=(LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.));
#4=LENGTH_MEASURE_WITH_UNIT(POSITIVE_LENGTH_MEASURE(0.13),#3);
#5=CURVE_STYLE(' ',#2,#4,#1);
#6=PRESENTATION_STYLE_ASSIGNMENT((#5));
#7=PRESENTATION_STYLE_ASSIGNMENT((NULL_STYLE(.NULL.)));
#8=DIRECTION(' ',(1.,0.));
#10=CARTESIAN_POINT(' ',(0.,0.));
#11=AXIS2_PLACEMENT_2D(' ',#10,#8);
#12=(GEOMETRIC_REPRESENTATION_CONTEXT(2) GLOBAL_UNIT_ASSIGNED_CONTEXT((#3,#14)) REPRESENTATION_CONTEXT('SAF','2D'));
#13=VECTOR(' ',#8,1.);
#14=(NAMED_UNIT(*) PLANE_ANGLE_UNIT() SI_UNIT($,.RADIAN.));
#20=CARTESIAN_POINT(' ',(100.,0.));
#21=LINE(' ',#10,#13);
#22=TRIMMED_CURVE(' ',#21,(#10),(#20),.T.,.CARTESIAN.);
#23=ANNOTATION_CURVE_OCCURRENCE(' ',(#6),#22);
#24=CARTESIAN_POINT(' ',(50.,25.));
#25=AXIS2_PLACEMENT_2D(' ',#24,$);
#26=CIRCLE(' ',#25,10.);
#27=ANNOTATION_CURVE_OCCURRENCE(' ',(#6),#26);
#44=DRAUGHTING_SUBFIGURE_REPRESENTATION('$$SXF_G_$$ATRF$$10$${saf_name}',(#23,#11),#12);
#45=SYMBOL_REPRESENTATION_MAP(#11,#44);
#46=SYMBOL_TARGET(' ',#11,1.,1.);
#47=(ANNOTATION_SYMBOL() GEOMETRIC_REPRESENTATION_ITEM() MAPPED_ITEM(#45,#46) REPRESENTATION_ITEM(' '));
#48=(ANNOTATION_OCCURRENCE() ANNOTATION_SUBFIGURE_OCCURRENCE() ANNOTATION_SYMBOL_OCCURRENCE() DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM() REPRESENTATION_ITEM(' ') STYLED_ITEM((#7),#47));
#120=PRESENTATION_LAYER_ASSIGNMENT('L',' ',(#23,#27,#48));
#121=DRAWING_SHEET_REVISION('A3_horizontal',(#48,#27,#122),#12,'01');
#122=PLANAR_BOX(' ',420.,297.,#11);
#123=PRESENTATION_SIZE(#121,#122);
#124=DRAWING_DEFINITION('SAF-10',$);
#125=DRAUGHTING_DRAWING_REVISION('01',#124,$);
#126=DRAWING_SHEET_REVISION_USAGE(#121,#125,'01');
#127=DRAUGHTING_TITLE((#125),'JAPANESE','SAF verification');
#128=PRESENTATION_LAYER_USAGE(#120,#121);
ENDSEC;
END-ISO-10303-21;
""".encode("ascii")


def saf(drawing_name):
    return f"""<?xml version="1.0" encoding="UTF-8"?>
<SxfAttributeXML version="3.1" date="2026-11-16" sxfFile="{drawing_name}" application="zumen verification">
  <AttributeSet name="検証セット" version="1.0" designedBy="zumen">1</AttributeSet>
  <Figure id="10" name="検証線分">
    <AttrSetRef id="1">
      <AttrGroup name="測点">
        <Attr name="測点名" type="STR">No.1</Attr>
        <Attr name="現地盤高" type="LEN" unit="m">102.0400</Attr>
      </AttrGroup>
    </AttrSetRef>
  </Figure>
</SxfAttributeXML>
""".encode("utf8")
