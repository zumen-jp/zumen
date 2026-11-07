import React, { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { Playground } from "./playground.jsx";

createRoot(document.querySelector("#root")).render(<StrictMode><Playground /></StrictMode>);
