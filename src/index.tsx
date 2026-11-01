/* @refresh reload */
import { render } from "solid-js/web";
import "./styles.css";
import App from "./App";

if (import.meta.env.VITE_ARTIFACT) document.documentElement.classList.add("preview");
render(() => <App />, document.getElementById("root")!);
