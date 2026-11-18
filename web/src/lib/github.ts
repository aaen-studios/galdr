export interface Asset {
  name: string;
  browser_download_url: string;
  size: number;
}

export interface Release {
  tag_name: string;
  html_url: string;
  assets: Asset[];
  body: string;
  published_at: string;
}

export async function getRelease(): Promise<Release | null> {
  try {
    const headers: Record<string, string> = {
      Accept: "application/vnd.github.v3+json",
    };
    const token = process.env.GITHUB_TOKEN;
    if (token) {
      headers.Authorization = `Bearer ${token}`;
    }

    const res = await fetch(
      "https://api.github.com/repos/aaen-studios/galdr/releases/latest",
      { headers, next: { revalidate: 3600 } }
    );
    if (!res.ok) {
      console.error("GitHub API returned", res.status, await res.text().catch(() => ""));
      return null;
    }
    return res.json();
  } catch (e) {
    console.error("GitHub API fetch failed:", e);
    return null;
  }
}

export async function getAllReleases(): Promise<Release[]> {
  try {
    const headers: Record<string, string> = {
      Accept: "application/vnd.github.v3+json",
    };
    const token = process.env.GITHUB_TOKEN;
    if (token) {
      headers.Authorization = `Bearer ${token}`;
    }

    const res = await fetch(
      "https://api.github.com/repos/aaen-studios/galdr/releases?per_page=5",
      { headers, next: { revalidate: 3600 } }
    );
    if (!res.ok) {
      console.error("GitHub API returned", res.status, await res.text().catch(() => ""));
      return [];
    }
    return res.json();
  } catch (e) {
    console.error("GitHub API fetch failed:", e);
    return [];
  }
}

/** Fetch a specific page of releases (for client-side infinite scroll). */
export async function getReleasesPage(page: number): Promise<Release[]> {
  try {
    const res = await fetch(
      `https://api.github.com/repos/aaen-studios/galdr/releases?per_page=5&page=${page}`,
      { headers: { Accept: "application/vnd.github.v3+json" } }
    );
    if (!res.ok) {
      console.error("GitHub API returned", res.status);
      return [];
    }
    return res.json();
  } catch (e) {
    console.error("GitHub API fetch failed:", e);
    return [];
  }
}

export function matchAsset(
  assets: Asset[],
  patterns: string[],
  exclude: string[] = []
): Asset | undefined {
  return assets.find((a) =>
    patterns.some((p) => a.name.toLowerCase().includes(p)) &&
    !exclude.some((e) => a.name.toLowerCase().includes(e))
  );
}
