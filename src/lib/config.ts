// Build-time feature flags. Real values come from Vite env (VITE_*) at build time.
// Secrets (CurseForge key, Sentry DSN, MSA client ID) are NEVER read here; they
// live in the Rust backend only. The frontend just asks the backend whether a
// feature is available.

export const APP_VERSION = "0.1.0";

/** Phase 7 monetization. Reserved slots render nothing while false. */
export const ADS_ENABLED = import.meta.env.VITE_ADS_ENABLED === "true";

/** True when running inside the Tauri shell (vs. the browser mockup). */
export const IS_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const MODRINTH_USER_AGENT = `loansharkz/HaloClient/${APP_VERSION} (github.com/loansharkz/haloclient)`;
