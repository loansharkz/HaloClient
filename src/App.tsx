import { HashRouter, MemoryRouter, Route, type RouteSectionProps } from "@solidjs/router";
import { lazy, onMount } from "solid-js";
import { Background, Sidebar, TitleBar, Toasts } from "./components/Shell";
import { CrashDialog, FirstRunBanner } from "./components/Overlays";
import { init } from "./lib/store";
import Home from "./pages/Home";

// Home is eager (first paint); everything else is lazy-loaded.
const Library = lazy(() => import("./pages/Library"));
const InstancePage = lazy(() => import("./pages/InstancePage"));
const Browse = lazy(() => import("./pages/Browse"));
const Project = lazy(() => import("./pages/Project"));
const Accounts = lazy(() => import("./pages/Accounts"));
const Settings = lazy(() => import("./pages/Settings"));

function Layout(props: RouteSectionProps) {
  return (
    <div class="relative h-full flex flex-col overflow-hidden">
      <Background />
      <div class="relative z-10 flex flex-col h-full">
        <TitleBar />
        <div class="flex flex-1 min-h-0 gap-3 px-3 pb-3">
          <Sidebar />
          <main class="flex-1 min-w-0 scroll pr-1" id="main">{props.children}</main>
        </div>
      </div>
      <Toasts />
      <CrashDialog />
      <FirstRunBanner />
    </div>
  );
}

export default function App() {
  onMount(() => init());
  // The shareable preview runs inside a sandboxed frame that drops "#/path"
  // hashes, so it routes in memory instead.
  const Router = import.meta.env.VITE_ARTIFACT ? MemoryRouter : HashRouter;
  return (
    <Router root={Layout}>
      <Route path="/" component={Home} />
      <Route path="/library" component={Library} />
      <Route path="/instance/:id" component={InstancePage} />
      <Route path="/browse" component={Browse} />
      <Route path="/project/:source/:id" component={Project} />
      <Route path="/accounts" component={Accounts} />
      <Route path="/settings" component={Settings} />
    </Router>
  );
}
