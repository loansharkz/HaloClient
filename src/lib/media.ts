// Public media for projects and servers. In the mockup this comes from the
// build-time `virtual:mock-media` snapshot; in the real app the Rust backend
// supplies the same shapes (Modrinth API + native Server List Ping).
import media from "virtual:mock-media";

export const projectMedia = (slug?: string) => (slug ? media.projects[slug] : undefined);
export const serverMedia = (address?: string) => (address ? media.servers[address] : undefined);
export const hasLiveMedia = () => media.fetchedAt !== null;
