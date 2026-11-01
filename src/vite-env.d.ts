/// <reference types="vite/client" />
interface ImportMetaEnv {
  readonly VITE_ADS_ENABLED?: string;
  readonly VITE_ARTIFACT: boolean;
}

declare module "virtual:mock-media" {
  const media: {
    fetchedAt: string | null;
    projects: Record<string, { icon?: string; banner?: string; gallery: string[] }>;
    servers: Record<string, { icon?: string; motd: string[]; players?: { online: number; max: number } }>;
  };
  export default media;
}
