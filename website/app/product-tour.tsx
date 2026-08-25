/* eslint-disable @next/next/no-img-element -- These screenshots are pre-sized WebP assets with fixed dimensions. */

export const localmailUrl = "https://github.com/hherb/localmail";

export default function ProductTour() {
  return (
    <section className="section product-tour" id="in-action">
      <div className="section-heading tour-heading">
        <div>
          <p className="kicker">Bottie in action</p>
          <h2>
            Real work.
            <br />
            <em>Visible context.</em>
          </h2>
        </div>
        <p>
          Search the current web, revisit what mattered in earlier conversations, and read a private email archive—with
          every source and route kept in view.
        </p>
      </div>

      <div className="tour-grid">
        <article className="tour-card tour-card-wide">
          <figure className="product-screenshot">
            <img
              src="/screenshots/bottie-web-research.webp"
              width="2200"
              height="1434"
              alt="Bottie answering a current pricing question with five visible web sources in the context panel"
              loading="lazy"
            />
          </figure>
          <div className="tour-copy">
            <p className="feature-tag">CURRENT WEB RESEARCH</p>
            <h3>Answers with the receipts beside them.</h3>
            <p>
              Bottie can search and read the public web, then keep every source visible beside the answer so you can
              inspect where current information came from.
            </p>
          </div>
        </article>

        <article className="tour-card">
          <figure className="product-screenshot">
            <img
              src="/screenshots/bottie-memory.webp"
              width="2200"
              height="1434"
              alt="Bottie recalling conference fees from three visible conversation memories"
              loading="lazy"
            />
          </figure>
          <div className="tour-copy">
            <p className="feature-tag">CONVERSATION MEMORY</p>
            <h3>Recall that stays inspectable.</h3>
            <p>
              When a past conversation is useful, Bottie shows the recalled memories in context instead of hiding them
              behind the answer.
            </p>
          </div>
        </article>

        <article className="tour-card tour-card-email">
          <figure className="product-screenshot">
            <img
              src="/screenshots/bottie-email.webp"
              width="2200"
              height="1434"
              alt="Bottie finding registration fees by searching a private email archive through Localmail"
              loading="lazy"
            />
          </figure>
          <div className="tour-copy">
            <p className="feature-tag">OPTIONAL EMAIL INTEGRATION</p>
            <h3>Ask questions of your email archive.</h3>
            <p>
              Email search and reading require a separate installation of Localmail. Bottie connects only to the
              Localmail server you explicitly configure.
            </p>
            <a className="inline-link" href={localmailUrl} rel="noreferrer">
              Install Localmail from GitHub <span aria-hidden="true">↗</span>
            </a>
          </div>
        </article>
      </div>
    </section>
  );
}
