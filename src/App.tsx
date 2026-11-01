import { HashRouter, MemoryRouter, Route, type RouteSectionProps } from "@solidjs/router";
import { lazy } from "solid-js";
import { Aurora, Sidebar, TitleBar, Toasts } from "./components/Shell";
import { initThemeSync } from "./lib/store";
import Home from "./pages/Home";

// Home is eager (first paint); everything else is lazy-loaded.
const Instances = lazy(() => import("./pages/Instances"));
const InstanceDetail = lazy(() => import("./pages/InstanceDetail"));
const Browse = lazy(() => import("./pages/Browse"));
const Accounts = lazy(() => import("./pages/Accounts"));
const Settings = lazy(() => import("./pages/Settings"));

function Layout(props: RouteSectionProps) {
  return (
    <div class="relative h-full flex flex-col overflow-hidden" style={{ "border-radius": "inherit" }}>
      <Aurora />
      <div class="relative z-10 flex flex-col h-full">
        <TitleBar />
        <div class="flex flex-1 min-h-0 gap-3 px-3 pb-3">
          <Sidebar />
          <main class="flex-1 min-w-0 scroll pr-1">{props.children}</main>
        </div>
      </div>
      <Toasts />
    </div>
  );
}

export default function App() {
  initThemeSync();
  // The shareable preview runs inside a sandboxed frame that drops "#/path"
  // hashes, so it routes in memory instead.
  const Router = import.meta.env.VITE_ARTIFACT ? MemoryRouter : HashRouter;
  return (
    <Router root={Layout}>
      <Route path="/" component={Home} />
      <Route path="/instances" component={Instances} />
      <Route path="/instances/:id" component={InstanceDetail} />
      <Route path="/browse" component={Browse} />
      <Route path="/accounts" component={Accounts} />
      <Route path="/settings" component={Settings} />
    </Router>
  );
}
