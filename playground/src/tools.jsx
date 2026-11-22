import React from "react";

const paths = {
  open: "M3 18V6h6l2 3h9v2 M3 18l3-7h16l-3 8H3z",
  select: "M5 3l14 10-7 1-3 7z",
  pan: "M4 13l4 5c1 2 3 3 6 3 4 0 6-3 6-7V8a1.5 1.5 0 0 0-3 0v3-6a1.5 1.5 0 0 0-3 0v6-7a1.5 1.5 0 0 0-3 0v7-5a1.5 1.5 0 0 0-3 0v8l-2-2c-1-1-3 0-2 1z",
  measure: "M3 15L15 3l6 6L9 21z M7 12l3 3m0-6 2 2m1-5 3 3",
  fit: "M9 3H3v6m12-6h6v6M3 15v6h6m12-6v6h-6 M8 8h8v8H8z",
  zoomIn: "M16 16l5 5 M18 10a8 8 0 1 1-16 0 8 8 0 0 1 16 0 M6.5 10h7m-3.5-3.5v7",
  zoomOut: "M16 16l5 5 M18 10a8 8 0 1 1-16 0 8 8 0 0 1 16 0 M6.5 10h7",
  layers: "M2 8l10-5 10 5-10 5z M3 12l9 5 9-5m-18 5 9 5 9-5",
  diagnostics: "M12 3L2 20h20z M12 9v5m0 2v1",
  review: "M4 4h16v12H9l-5 4z M8 8h8m-8 4h5",
  export: "M4 14v6h16v-6 M12 15V3m-5 5 5-5 5 5",
  info: "M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0 M12 11v6m0-11v1",
  close: "M6 6l12 12M18 6 6 18",
  sheet: "M5 2h10l4 4v16H5z M14 2v5h5 M8 11h7m-7 4h7",
};

export function Icon({ name }) {
  return <svg className="cad-icon" viewBox="0 0 24 24" width="20" height="20" fill="none"
    stroke="currentColor" strokeWidth="1.65" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <path d={paths[name]}/>
  </svg>;
}

export function ToolButton({ icon, children, ...props }) {
  return <button type="button" className="cad-tool" {...props}><Icon name={icon}/><span>{children}</span></button>;
}
