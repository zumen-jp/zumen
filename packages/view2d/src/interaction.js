import { SCENE_NONE } from "@zumen-jp/zumen";
import { end, rows, valid } from "./validate.js";

const decoder = new TextDecoder();
const EMPTY = Object.freeze([]);

export const elementKey = (element) => JSON.stringify([element.id, element.path.map(({ id, column, row }) => [id, column, row])]);

/** 幾何の列は写さず、選択に使う ID と配置の道筋の索引だけを作る。 */
export function indexElements(prepared) {
  const c = prepared.columns;
  const stringAt = (name, i) => decoder.decode(c.strings.subarray(c[name][i * 2], c[name][i * 2] + c[name][i * 2 + 1]));
  const elements = [], byKey = new Map(), indices = new Map();
  for (let entity = 0; entity < rows(c, "entity_ids"); entity++) {
    const path = [];
    for (let i = c.entity_paths[entity]; i < end(c.entity_paths, entity, rows(c, "step_ids")); i++) {
      path.push(Object.freeze({ id: stringAt("step_ids", i), column: c.step_cells[i * 2], row: c.step_cells[i * 2 + 1] }));
    }
    const ref = Object.freeze({ id: stringAt("entity_ids", entity), path: Object.freeze(path) });
    const key = elementKey(ref);
    if (!byKey.has(key)) { byKey.set(key, ref); indices.set(ref, []); }
    const canonical = byKey.get(key);
    elements.push(canonical);
    indices.get(canonical).push(entity);
  }
  const layers = [], layerIndices = new Map();
  for (let i = 0; i < rows(c, "layer_ids"); i++) {
    const id = stringAt("layer_ids", i);
    if (!layerIndices.has(id)) { layers.push(id); layerIndices.set(id, []); }
    layerIndices.get(id).push(i);
  }
  return { elements: Object.freeze(elements), byKey, indices, layers: Object.freeze(layers), layerIndices };
}

export const emptyInteraction = () => Object.freeze({ hiddenLayers: EMPTY, selection: EMPTY, hovered: null, highlights: EMPTY });

/** 利用側の参照を、現在のシーンが持つ凍結した参照へ戻す。 */
export function references(values, index, limits) {
  valid(Array.isArray(values) && values.length <= limits.stateItems, "selection");
  const out = new Map();
  let work = 0;
  for (let i = 0; i < values.length; i++) {
    const ref = values[i];
    valid(ref && typeof ref.id === "string" && Array.isArray(ref.path), "element");
    work += ref.id.length + ref.path.length + 1;
    valid(work <= limits.work, "limits.work");
    const path = [];
    for (let j = 0; j < ref.path.length; j++) {
      const step = ref.path[j];
      valid(step && typeof step.id === "string" && [step.column, step.row].every((n) => Number.isInteger(n) && n >= 0 && n <= 0xffffffff), "element.path");
      work += step.id.length;
      valid(work <= limits.work, "limits.work");
      path.push({ id: step.id, column: step.column, row: step.row });
    }
    const key = elementKey({ id: ref.id, path });
    valid(index?.byKey.has(key), "element.unknown");
    out.set(key, index.byKey.get(key));
  }
  return Object.freeze([...out.values()]);
}

export function hiddenLayers(values, index, limits) {
  valid(Array.isArray(values) && values.length <= limits.stateItems, "hiddenLayers");
  const out = new Set();
  for (let i = 0; i < values.length; i++) {
    valid(typeof values[i] === "string" && index?.layerIndices.has(values[i]), "layer.unknown");
    out.add(values[i]);
  }
  return Object.freeze([...out]);
}

/** 配置ごとの見え方を先に確かめ、失敗した変更を現在の画面へ反映しない。 */
export function appearance(state, index, settings, limits) {
  const hidden = new Set(), colors = new Map();
  let count = state.selection.length + state.hiddenLayers.length + (state.hovered ? 1 : 0);
  for (const group of state.highlights) count += group.elements.length + 1;
  valid(count <= limits.stateItems, "limits.stateItems");
  let work = 0;
  const charge = () => valid(++work <= limits.work, "limits.work");
  for (const id of state.hiddenLayers) for (const layer of index.layerIndices.get(id)) { charge(); hidden.add(layer); }
  function assign(ref, color) {
    for (const entity of index.indices.get(ref)) { charge(); colors.set(entity, color); }
  }
  for (const group of state.highlights) for (const ref of group.elements) assign(ref, group.color);
  for (const ref of state.selection) assign(ref, settings.selectionColor);
  if (state.hovered) assign(state.hovered, settings.hoverColor);
  return { hidden, colors };
}

export function unitVisible(columns, unit, display) {
  const layer = columns.unit_layers[unit];
  return layer === SCENE_NONE || !display?.hidden.has(layer);
}
