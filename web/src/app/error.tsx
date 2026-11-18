"use client";

export default function Error({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  return (
    <main>
      <header className="title-page">
        <div className="kicker">the spell misfired</div>
        <h1 className="wordmark">
          err<em>or</em>
        </h1>
        <p className="sub">
          Something went wrong while conjuring this page. Speak the incantation again, or return to
          the title page.
        </p>
        {error.digest ? <p className="colophon-note">trace: {error.digest}</p> : null}
        <div className="engraved-rule" aria-hidden="true" />
        <div>
          <button className="seal" onClick={() => reset()}>
            Cast again
          </button>
        </div>
      </header>
    </main>
  );
}
