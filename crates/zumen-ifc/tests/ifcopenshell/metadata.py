"""5 種類の元属性を実行 API で照らす。スキーマと実装の照会は行わない。"""
import json
import math
import sys
from pathlib import Path

import ifcopenshell


def scalar(value):
    if value is None:
        return None
    if isinstance(value, (tuple, list)):
        return [scalar(v) for v in value]
    return getattr(value, 'wrappedValue', value)


def scale(unit):
    if unit.is_a('IfcSIUnit'):
        value = {'MILLI': .001, 'KILO': 1000, None: 1}[unit.Prefix]
        if unit.Name == 'SQUARE_METRE':
            value **= 2
        elif unit.Name == 'CUBIC_METRE':
            value **= 3
        return value * (.001 if unit.Name == 'GRAM' else 1)
    if unit.is_a('IfcConversionBasedUnit'):
        return scalar(unit.ConversionFactor.ValueComponent) * scale(unit.ConversionFactor.UnitComponent)
    if unit.is_a('IfcDerivedUnit'):
        return math.prod(scale(e.Unit) ** e.Exponent for e in unit.Elements)
    raise AssertionError(unit.is_a())


def unit_value(unit):
    if unit is None:
        return None
    if unit.is_a('IfcSIUnit'):
        return [unit.UnitType, unit.Prefix, unit.Name, None]
    if unit.is_a('IfcDerivedUnit'):
        name = getattr(unit, 'Name', None) or ' * '.join(
            ((getattr(e.Unit, 'Prefix', None) + ' ') if getattr(e.Unit, 'Prefix', None) else '')
            + e.Unit.Name + '^' + str(e.Exponent) for e in unit.Elements)
        return [unit.UnitType, None, unit.UserDefinedType if unit.UnitType == 'USERDEFINED' else name, scale(unit)]
    return [unit.UnitType, None, unit.Name, scale(unit)]


def check_unit(actual, expected):
    if actual is None or expected is None:
        assert actual == expected, (actual, expected)
        return
    assert actual[:3] == expected[:3], (actual, expected)
    if expected[3] is None:
        assert actual[3] is None
    else:
        assert math.isclose(actual[3], expected[3], rel_tol=1.e-14), (actual, expected)


def definitions(model, owner):
    if owner.is_a('IfcTypeObject'):
        return owner.HasPropertySets or ()
    if owner.is_a('IfcMaterial'):
        if model.schema == 'IFC2X3':
            return [p for p in model.by_type('IfcExtendedMaterialProperties') if p.Material == owner]
        return owner.HasProperties or ()
    return [r.RelatingPropertyDefinition for r in owner.IsDefinedBy if r.is_a('IfcRelDefinesByProperties')]


def compare(path):
    model = ifcopenshell.open(str(path.with_suffix('.ifc')))
    data = json.loads(path.read_text(encoding='utf-8'))
    units = model.by_type('IfcProject')[0].UnitsInContext.Units
    length = next(u for u in units if u.UnitType == 'LENGTHUNIT')
    flow = next(u for u in units if u.UnitType == 'VOLUMETRICFLOWRATEUNIT')
    count = 0
    for value in data['properties']:
        owner = model.by_id(value['owner'])
        definition = next(d for d in definitions(model, owner) if d.Name == value['set'])
        fields = getattr(definition, 'HasProperties', None) or getattr(definition, 'Properties', None) or getattr(definition, 'ExtendedProperties', None)
        if fields:
            prop = next(p for p in fields if p.Name == value['name'])
            raw = getattr(prop, 'NominalValue', None)
            if prop.is_a('IfcPropertyListValue'):
                raw = prop.ListValues
            if prop.is_a('IfcPropertyEnumeratedValue'):
                raw = prop.EnumerationValues
                unit = prop.EnumerationReference.Unit if prop.EnumerationReference else None
            else:
                unit = prop.Unit
            if unit is None and value['unit'] is not None:
                unit = flow if value['unit'][0] == 'VOLUMETRICFLOWRATEUNIT' else length
            assert scalar(raw) == value['value'], value
        else:
            assert getattr(definition, value['name']) == value['value'], value
            unit = length if value['unit'] else None
        check_unit(value['unit'], unit_value(unit))
        count += 1
    for value in data['details']:
        prop = model.by_id(value['source'])
        if value['parent'] is not None:
            assert prop in model.by_id(value['parent']).HasProperties
        if 'usage' in value:
            assert prop.UsageName == value['usage']
            count += 1
            continue
        if 'lower' in value:
            assert scalar(prop.LowerBoundValue) == value['lower']
            assert scalar(prop.UpperBoundValue) == value['upper']
            assert scalar(getattr(prop, 'SetPointValue', None)) == value['setPoint']
            unit = prop.Unit or length
        elif prop.is_a('IfcPropertyEnumeratedValue'):
            assert scalar(prop.EnumerationValues) == value['values']
            assert scalar(prop.EnumerationReference.EnumerationValues) == value['choices']
            assert prop.EnumerationReference.Name == value['choiceName']
            unit = prop.EnumerationReference.Unit
        elif prop.is_a('IfcPropertyListValue'):
            assert scalar(prop.ListValues) == value['values']
            unit = prop.Unit or (length if value['unit'] else None)
        else:
            assert scalar(prop.NominalValue) == value['value']
            unit = prop.Unit or length
        check_unit(value['unit'], unit_value(unit))
        count += 1
    for value in data['materials']:
        owner = model.by_id(value['owner'])
        material = next(r.RelatingMaterial for r in owner.HasAssociations if r.is_a('IfcRelAssociatesMaterial'))
        assert getattr(material, 'LayerSetDirection', None) == value['direction']
        assert getattr(material, 'OffsetFromReferenceLine', None) == value['offset']
        if material.is_a('IfcMaterialLayerSetUsage'):
            parts = material.ForLayerSet.MaterialLayers
        elif material.is_a('IfcMaterialConstituentSet'):
            parts = material.MaterialConstituents
        else:
            parts = material.MaterialProfiles
        expected = [{'material': p.Material.id() if p.Material else None, 'name': getattr(p, 'Name', None), 'thickness': getattr(p, 'LayerThickness', None)} for p in parts]
        assert expected == value['parts'], (expected, value)
        count += len(parts)
    expected = []
    for rel in model.by_type('IfcRelConnectsElements'):
        expected += [[rel.RelatingElement.id(), rel.is_a(), rel.RelatedElement.id()], [rel.RelatedElement.id(), rel.is_a(), rel.RelatingElement.id()]]
    for rel in model.by_type('IfcRelCoversBldgElements'):
        for related in rel.RelatedCoverings:
            expected += [[related.id(), rel.is_a(), rel.RelatingBuildingElement.id()], [rel.RelatingBuildingElement.id(), rel.is_a(), related.id()]]
    for rel in model.by_type('IfcRelAssignsToActor'):
        expected += [[r.id(), rel.is_a(), rel.RelatingActor.id()] for r in rel.RelatedObjects]
    assert sorted(expected) == sorted(data['connections'])
    count += len(expected)
    print(f'{path.stem}: 5 種類の元属性・参照 {count} 件が一致（倍率の相対許容差 1e-14）')
    return count


if __name__ == '__main__':
    paths = sorted(Path(sys.argv[1]).glob('*.json'))
    assert [p.stem for p in paths] == ['IFC2X3', 'IFC4', 'IFC4X3_ADD2']
    total = sum(compare(path) for path in paths)
    print(f'IfcOpenShell {ifcopenshell.version}: 合成の元属性・参照 {total} 件が一致')
