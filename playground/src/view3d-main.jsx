import React, { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { Viewer3D } from "./view3d.jsx";

createRoot(document.querySelector("#root")).render(<StrictMode><Viewer3D /></StrictMode>);
