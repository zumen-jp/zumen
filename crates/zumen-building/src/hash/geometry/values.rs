//! グラフの参照を含まない元値の符号化。既存コンポーネントの符号化には触れない。
use super::*;

macro_rules! fields {
    ($($ty:ident { $($field:ident),* $(,)? });* $(;)?) => {$(
        impl Encode for $ty { fn encode(&self, out: &mut Canon<'_>) { $(self.$field.encode(out);)* } }
    )*};
}
macro_rules! tags {
    ($($ty:ident { $($variant:ident = $tag:literal),* $(,)? });* $(;)?) => {$(
        impl Encode for $ty { fn encode(&self, out: &mut Canon<'_>) { match self { $(Self::$variant => ($tag as u8).encode(out),)* } } }
    )*};
}
fields! {
    AxisPlacement2 { location, reference_direction };
    AxisPlacement3 { location, axis, reference_direction };
    CurvePosition { location, axis, reference_direction };
    RevolutionAxis { location, direction };
    CartesianMapping3 { local_origin, axis1, axis2, axis3, scale };
    ShapeMapping3 { origin, target };
    GeometricContext { identifier, kind, dimension, precision, world_coordinate_system, true_north };
    SubContext { identifier, kind, target_scale, target_view, user_defined_target_view };
    RepresentationContext { geometric, view };
    SurfaceColour { name, rgb };
    SurfaceRendering { diffuse, transmission, diffuse_transmission, reflection, specular, highlight, method };
    SurfaceShading { colour, transparency, rendering };
    SurfaceStyle { name, side, shading };
    LayerState { on, frozen, blocked };
    PresentationLayer { name, description, identifier, state };
    GeometrySurfaceStyle { definition };
    GeometryLayer { definition, styles, level };
    GeometryPresentation { name, direct, layers, layer_source, surface_source };
    GridAxisDefinition { set, tag, same_sense };
    IndexedPolygon { boundary, holes };
}
tags! {
    BooleanOperation { Union = 0, Intersection = 1, Difference = 2 };
    CurveTransition { Discontinuous = 0, Continuous = 1, SameGradient = 2, SameGradientSameCurvature = 3 };
    TrimmingPreference { Cartesian = 0, Parameter = 1, Unspecified = 2 };
    FaceModelKind { FacetedSolid = 0, ShellSurface = 1, FaceSurface = 2 };
    EnclosureCoordinates { BaseSurface = 0, Object = 1 };
    TargetView { Graph = 0, Sketch = 1, Model = 2, Plan = 3, ReflectedPlan = 4, Section = 5, Elevation = 6, UserDefined = 7, NotDefined = 8 };
    SurfaceSide { Positive = 0, Negative = 1, Both = 2 };
    ReflectanceMethod { Blinn = 0, Flat = 1, Glass = 2, Matt = 3, Metal = 4, Mirror = 5, Phong = 6, Physical = 7, Plastic = 8, Strauss = 9, NotDefined = 10 };
    LayerAssignmentLevel { None = 0, Representation = 1, Item = 2 };
    GridLayout { Rectangular = 0, Radial = 1, Triangular = 2, Irregular = 3, UserDefined = 4, NotDefined = 5 };
}

impl Encode for [u32; 3] {
    fn encode(&self, out: &mut Canon<'_>) {
        for v in self {
            v.encode(out);
        }
    }
}
impl<T: Encode> Encode for Acquired<T> {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            Self::Value(v) => {
                0u8.encode(out);
                v.encode(out);
            }
            Self::Unsupported => 1u8.encode(out),
        }
    }
}
impl Encode for Coordinates {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            Self::Two(v) => {
                2u8.encode(out);
                v.encode(out);
            }
            Self::Three(v) => {
                3u8.encode(out);
                v.encode(out);
            }
        }
    }
}
impl Encode for ContextPlacement {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            Self::Plane(v) => {
                2u8.encode(out);
                v.encode(out);
            }
            Self::Space(v) => {
                3u8.encode(out);
                v.encode(out);
            }
        }
    }
}
impl Encode for CurveTrim {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            Self::Point(v) => {
                0u8.encode(out);
                v.encode(out);
            }
            Self::Parameter(v) => {
                1u8.encode(out);
                v.encode(out);
            }
        }
    }
}
impl Encode for IndexedSegment {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            Self::Line(v) => {
                0u8.encode(out);
                v.encode(out);
            }
            Self::Arc(v) => {
                1u8.encode(out);
                v.encode(out);
            }
        }
    }
}
impl Encode for MappingScale3 {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            Self::Uniform(v) => {
                0u8.encode(out);
                v.encode(out);
            }
            Self::NonUniform { x, y, z } => {
                1u8.encode(out);
                x.encode(out);
                y.encode(out);
                z.encode(out);
            }
        }
    }
}
impl Encode for ColourOrFactor {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            Self::Colour(v) => {
                0u8.encode(out);
                v.encode(out);
            }
            Self::Factor(v) => {
                1u8.encode(out);
                v.encode(out);
            }
        }
    }
}
impl Encode for SpecularHighlight {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            Self::Exponent(v) => {
                0u8.encode(out);
                v.encode(out);
            }
            Self::Roughness(v) => {
                1u8.encode(out);
                v.encode(out);
            }
        }
    }
}
impl Encode for SurfaceStyleSelection {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            Self::None => 0u8.encode(out),
            Self::Direct => 1u8.encode(out),
            Self::Layer(v) => {
                2u8.encode(out);
                v.encode(out);
            }
            Self::MultipleLayers => 3u8.encode(out),
        }
    }
}
