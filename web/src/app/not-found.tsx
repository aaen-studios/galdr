import Link from "next/link";

export default function NotFound() {
  return (
    <main>
      <header className="title-page">
        <div className="kicker">a page lost to time</div>
        <h1 className="wordmark">
          4<em>0</em>4
        </h1>
        <p className="sub">
          This folio was never inscribed — or has crumbled away. Return to the title page and try
          again.
        </p>
        <div className="engraved-rule" aria-hidden="true" />
        <div>
          <Link className="seal" href="/">
            Back to the book
          </Link>
        </div>
      </header>
    </main>
  );
}
