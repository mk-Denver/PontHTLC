export const loc = $state({
  pathname: "/",
  parts: [] as string[],
});

function readHash(): string {
  const raw = decodeURIComponent(location.hash.replace(/^#/, "") || "/");
  return raw.startsWith("/") ? raw : `/${raw}`;
}

export function syncRoute() {
  const pathname = readHash();
  loc.pathname = pathname;
  loc.parts = pathname.split("/").filter(Boolean);
}

export function go(path: string) {
  const next = path.startsWith("/") ? path : `/${path}`;
  if (readHash() === next) {
    syncRoute();
    return;
  }
  location.hash = next;
}

export function param(index: number): string {
  return loc.parts[index] ?? "";
}

export function startRouter() {
  syncRoute();
  window.addEventListener("hashchange", syncRoute);
  return () => window.removeEventListener("hashchange", syncRoute);
}
