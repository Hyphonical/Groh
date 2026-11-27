import { mount } from "svelte";
import App from "./App.svelte";
import "./lib/palette.css";

mount(App, { target: document.getElementById("app") });
