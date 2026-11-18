import { getAllReleases, getRelease, matchAsset, type Asset } from "@/lib/github";
import Changelog from "@/components/changelog";
import Reveal from "@/components/reveal";

/* ── helpers ─────────────────────────────────── */

function formatSize(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${bytes} B`;
}

type Binding = {
  os: string;
  patterns: string[];
};

const BINDINGS: Binding[] = [
  { os: "Windows", patterns: ["x64-setup.exe", "x86_64.msi", ".msi", ".exe"] },
  { os: "macOS", patterns: [".dmg"] },
  { os: "Linux", patterns: [".appimage", ".deb", ".rpm"] },
];

function findBinding(assets: Asset[], binding: Binding): Asset | undefined {
  // Prefer the most specific pattern first; never match detached signatures
  // or update sidecars.
  for (const pattern of binding.patterns) {
    const hit = matchAsset(assets, [pattern], [".sig", ".zip", "update.json"]);
    if (hit) return hit;
  }
  return undefined;
}

/* ── page ────────────────────────────────────── */

export default async function Home() {
  const [latest, releases] = await Promise.all([getRelease(), getAllReleases()]);
  const version = latest?.tag_name.replace(/^v/, "") ?? "";

  return (
    <>
      <nav className="primary" aria-label="Primary">
        <a href="#spells">Spells</a>
        <a href="#forge">The Forge</a>
        <a href="#downloads">Downloads</a>
        <a href="#changelog">Changelog</a>
      </nav>

      <main>
        {/* ═══ TITLE PAGE ═══ */}
        <header className="title-page" id="top">
          <div className="kicker">
            an incantation for media{version ? ` · v${version}` : ""}
          </div>
          <h1 className="wordmark">
            gal<em>dr</em>
          </h1>
          <p className="sub">
            A desktop grimoire around FFmpeg — transmute, compress and inscribe video, audio, and
            image files without leaving the page.
          </p>

          <dl className="gloss">
            <dt>
              <em>galðr</em>, Old Norse
            </dt>
            <dd>
              a magical incantation · plural <em>galðar</em>: sorcery, witchcraft — a spell you
              speak over a thing until it changes shape
            </dd>
          </dl>

          <div className="engraved-rule" aria-hidden="true" />

          <nav className="capitula" aria-label="Folios">
            <h2>Capitulum — the folios</h2>
            <a href="#spells">
              <span>I · The Spells</span>
              <i>p. 01</i>
            </a>
            <a href="#forge">
              <span>II · The Forge</span>
              <i>p. 02</i>
            </a>
            <a href="#downloads">
              <span>III · Take the Book Home</span>
              <i>p. 03</i>
            </a>
            <a href="#changelog">
              <span>IV · Index of Incantations</span>
              <i>p. 04</i>
            </a>
          </nav>

          <div>
            <a className="seal" href="#downloads">
              Open the downloads
            </a>
          </div>
          <p className="colophon-note">built for windows · macos · linux — tauri · react · rust</p>
        </header>

        {/* ═══ FOLIO I: SPELLS / FEATURES ═══ */}
        <section id="spells" aria-labelledby="spells-title">
          <Reveal className="folio-head">
            <div className="folio-num">FOLIO I</div>
            <h2 className="folio-title" id="spells-title">
              The Spells
            </h2>
            <p className="folio-latin">de transformatione et compressione</p>
          </Reveal>
          <div className="divider" aria-hidden="true">
            ✦ ─── ✦
          </div>

          <div className="folios two">
            <Reveal>
              <article className="entry">
                <span className="margin-note" aria-hidden="true">
                  <b>§1.1</b>transmutatio
                </span>
                <h3>Transmutation</h3>
                <p className="dropcap">
                  Hurl any file into the crucible and name its new form — mp4 to mkv, webm to avi,
                  png stacks to video. Every codec FFmpeg knows is at your fingertips, no command
                  line required.
                </p>
              </article>
            </Reveal>

            <Reveal>
              <article className="entry">
                <span className="margin-note" aria-hidden="true">
                  <b>§1.2</b>compressio
                </span>
                <h3>Compression</h3>
                <p className="dropcap">
                  Quality by the number of grains: set a CRF and watch the weight fall away.
                  Constant-quality, target-size, or full manual — each pass is verified before it
                  commits.
                </p>
              </article>
            </Reveal>

            <Reveal>
              <article className="entry">
                <span className="margin-note" aria-hidden="true">
                  <b>§1.3</b>subtilia
                </span>
                <h3>Subtitles</h3>
                <p className="dropcap">
                  Burn inscriptions into the frame, or bind them softly so they travel with the
                  file. SRT, ASS and VTT all accepted at the door.
                </p>
              </article>
            </Reveal>

            <Reveal>
              <article className="entry">
                <span className="margin-note" aria-hidden="true">
                  <b>§1.4</b>extractio
                </span>
                <h3>Extraction</h3>
                <p className="dropcap">
                  Pull the voice from the moving image — audio out as mp3, flac, or ogg — without
                  re-encoding what you did not touch.
                </p>
              </article>
            </Reveal>
          </div>
        </section>

        {/* ═══ FOLIO II: THE FORGE ═══ */}
        <section id="forge" aria-labelledby="forge-title">
          <div className="divider" aria-hidden="true">
            ✦ ─── ✦
          </div>
          <Reveal className="folio-head">
            <div className="folio-num">FOLIO II</div>
            <h2 className="folio-title" id="forge-title">
              The Forge
            </h2>
            <p className="folio-latin">de tempore fuscato — on the shaping of time</p>
          </Reveal>

          <Reveal>
            <article className="entry">
              <span className="margin-note" aria-hidden="true">
                <b>§2.0</b>forges
              </span>
              <p className="dropcap">
                When a spell is not enough, step into the forge: a timeline editor over raw FFmpeg
                operations. Cut, scale, crop and layer — every gesture compiles to a command you
                can read, copy, or run again.
              </p>
            </article>
          </Reveal>
        </section>

        {/* ═══ FOLIO III: DOWNLOADS ═══ */}
        <section id="downloads" aria-labelledby="dl-title">
          <div className="divider" aria-hidden="true">
            ✦ ─── ✦
          </div>
          <Reveal className="folio-head">
            <div className="folio-num">FOLIO III</div>
            <h2 className="folio-title" id="dl-title">
              Take the Book Home
            </h2>
            <p className="folio-latin">codices — three bindings, one text</p>
          </Reveal>

          <Reveal className="downloads">
            {latest
              ? BINDINGS.map((binding) => {
                  const asset = findBinding(latest.assets, binding);
                  return asset ? (
                    <a key={binding.os} className="dl-row" href={asset.browser_download_url}>
                      <span>
                        <span className="dl-os">{binding.os}</span>
                        <span className="dl-file">{asset.name}</span>
                      </span>
                      <span className="dl-meta">{formatSize(asset.size)} · sha256 verified</span>
                    </a>
                  ) : (
                    <div key={binding.os} className="dl-row dl-row-dim">
                      <span>
                        <span className="dl-os">{binding.os}</span>
                        <span className="dl-file">not yet bound in this release</span>
                      </span>
                      <span className="dl-meta">soon</span>
                    </div>
                  );
                })
              : BINDINGS.map((binding) => (
                  <a
                    key={binding.os}
                    className="dl-row"
                    href="https://github.com/aaen-studios/galdr/releases/latest"
                  >
                    <span>
                      <span className="dl-os">{binding.os}</span>
                      <span className="dl-file">the release index is unreachable — open github</span>
                    </span>
                    <span className="dl-meta">release page</span>
                  </a>
                ))}
          </Reveal>
        </section>

        {/* ═══ FOLIO IV: CHANGELOG INDEX ═══ */}
        <section id="changelog" aria-labelledby="cl-title">
          <div className="divider" aria-hidden="true">
            ✦ ─── ✦
          </div>
          <Reveal className="folio-head">
            <div className="folio-num">FOLIO IV</div>
            <h2 className="folio-title" id="cl-title">
              Index of Incantations
            </h2>
            <p className="folio-latin">a record of what the forge has spoken</p>
          </Reveal>

          <Changelog initialReleases={releases} />
        </section>

        {/* ═══ COLOPHON ═══ */}
        <footer>
          <div className="divider" aria-hidden="true">
            ✦ ─── ✦
          </div>
          <p className="colophon">
            galdr — set in Cormorant Garamond &amp; Source Serif · typeset for the web by{" "}
            <a href="https://aaenz.no" target="_blank" rel="noopener noreferrer">
              aaen studios
            </a>
            <br />
            source on{" "}
            <a
              href="https://github.com/aaen-studios/galdr"
              target="_blank"
              rel="noopener noreferrer"
            >
              github.com/aaen-studios/galdr
            </a>{" "}
            · free software, forever
          </p>
        </footer>
      </main>
    </>
  );
}
