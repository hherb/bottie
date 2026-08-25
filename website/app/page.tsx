/* eslint-disable @next/next/no-img-element -- These screenshots are pre-sized WebP assets with fixed dimensions. */

import ProductTour, { localmailUrl } from "./product-tour";

const githubUrl = "https://github.com/hherb/bottie";

export default function Home() {
  return (
    <main>
      <header className="site-header">
        <a className="brand" href="#top" aria-label="Bottie home">
          <span className="brand-mark" aria-hidden="true">
            <span className="brand-core" />
          </span>
          <span>bottie</span>
          <span className="preview-label">preview</span>
        </a>

        <nav aria-label="Primary navigation">
          <a href="#in-action">In action</a>
          <a href="#capabilities">Capabilities</a>
          <a href="#roadmap">Roadmap</a>
          <a className="nav-cta" href={githubUrl} rel="noreferrer">
            GitHub <span aria-hidden="true">↗</span>
          </a>
        </nav>
      </header>

      <section className="hero" id="top">
        <div className="hero-copy">
          <p className="eyebrow">
            <span /> Local-first AI, thoughtfully connected
          </p>
          <h1>
            Your context.
            <br />
            Your models.
            <br />
            <em>Your rules.</em>
          </h1>
          <p className="hero-intro">
            Bottie is a private desktop AI companion that remembers what matters, shows its work, and lets you choose
            what leaves your machine.
          </p>
          <div className="hero-actions">
            <a className="button button-primary" href="#capabilities">
              Explore Bottie <span>↓</span>
            </a>
            <a className="button button-secondary" href={githubUrl} rel="noreferrer">
              View the project <span>↗</span>
            </a>
          </div>
          <div className="hero-proof" aria-label="Bottie product principles">
            <span>
              <i className="proof-dot proof-local" /> Local by default
            </span>
            <span>
              <i className="proof-dot proof-visible" /> Visible context
            </span>
            <span>
              <i className="proof-dot proof-open" /> Provider flexible
            </span>
          </div>
        </div>

        <div className="product-stage">
          <div className="stage-glow" />
          <figure className="product-screenshot hero-screenshot">
            <img
              src="/screenshots/bottie-tools.webp"
              width="2200"
              height="1434"
              alt="Bottie listing available tools beside a visible privacy route"
            />
          </figure>
          <div className="context-card memory-card">
            <span className="card-icon">◎</span>
            <div>
              <strong>Tools you can see</strong>
              <small>Memory · web · email · files</small>
            </div>
          </div>
          <div className="context-card privacy-card">
            <span className="card-icon">✓</span>
            <div>
              <strong>Private route</strong>
              <small>Nothing left this Mac</small>
            </div>
          </div>
        </div>
      </section>

      <section className="provider-band" aria-label="Supported model providers">
        <p>One home for the models you choose</p>
        <div>
          <span>
            <i className="provider-dot omlx" /> oMLX
          </span>
          <span>
            <i className="provider-dot ollama" /> Ollama
          </span>
          <span>
            <i className="provider-dot openai" /> OpenAI-compatible
          </span>
          <span>
            <i className="provider-dot anthropic" /> Anthropic-compatible
          </span>
        </div>
      </section>

      <ProductTour />

      <section className="section capabilities" id="capabilities">
        <div className="section-heading">
          <div>
            <p className="kicker">Available in the developer preview</p>
            <h2>
              Capable by design.
              <br />
              <em>Careful by default.</em>
            </h2>
          </div>
          <p>
            Bottie brings serious AI capability into a desktop experience built around privacy, inspectability, and
            choice—not hidden automation.
          </p>
        </div>

        <div className="feature-grid">
          <article className="feature-card feature-memory">
            <div className="feature-number">01</div>
            <div className="memory-visual" aria-hidden="true">
              <div className="memory-search">
                ⌁ <span>Search your memory…</span>
              </div>
              <div className="memory-result result-one">
                <i />{" "}
                <span>
                  <strong>Launch direction</strong>
                  <small>Conversation · 94% match</small>
                </span>
              </div>
              <div className="memory-result result-two">
                <i />{" "}
                <span>
                  <strong>Product research</strong>
                  <small>Attached notes · 87% match</small>
                </span>
              </div>
            </div>
            <p className="feature-tag">PRIVATE MEMORY</p>
            <h3>Remembers—with your permission.</h3>
            <p>
              Bottie can search past conversations and attached documents, then shows exactly which memories it used.
              Exclude, remove, or forget at any time.
            </p>
          </article>

          <article className="feature-card feature-models">
            <div className="feature-number">02</div>
            <div className="model-orbit" aria-hidden="true">
              <span className="orbit-ring ring-one" />
              <span className="orbit-ring ring-two" />
              <span className="orbit-core">
                <i className="brand-core" />
              </span>
              <span className="orbit-node node-local">Local</span>
              <span className="orbit-node node-cloud">Cloud</span>
              <span className="orbit-node node-open">Open</span>
            </div>
            <p className="feature-tag">MODEL FREEDOM</p>
            <h3>Local when you want it. Connected when you need it.</h3>
            <p>
              Move between oMLX, Ollama, OpenAI-compatible, and Anthropic-compatible models. Bottie makes the active
              route visible before you send.
            </p>
          </article>

          <article className="feature-card feature-sources">
            <div className="feature-number">03</div>
            <div className="citation-visual" aria-hidden="true">
              <div className="citation-line wide" />
              <div className="citation-line medium" />
              <p>
                Current research supports a focused release.<sup>1</sup>
              </p>
              <div className="citation-card">
                <span>◎</span>
                <div>
                  <strong>Source inspected</strong>
                  <small>Web · cited in response</small>
                </div>
                <b>↗</b>
              </div>
            </div>
            <p className="feature-tag">RESEARCH WITH RECEIPTS</p>
            <h3>Search, read, and cite the web.</h3>
            <p>
              Native web search and page reading keep sources visible, citations connected to claims, and fetched
              content clearly marked as untrusted.
            </p>
          </article>

          <article className="feature-card feature-files">
            <div className="feature-number">04</div>
            <div className="file-visual" aria-hidden="true">
              <span className="file-chip">
                <i>PDF</i> field-notes.pdf <b>✓</b>
              </span>
              <span className="file-chip">
                <i>DOC</i> brief.docx <b>✓</b>
              </span>
              <span className="file-chip">
                <i>IMG</i> sketch.png <b>✓</b>
              </span>
            </div>
            <p className="feature-tag">FILES & VISION</p>
            <h3>Bring the work you already have.</h3>
            <p>
              Read PDFs, DOCX, Markdown, plain text, and images through bounded native extraction. Files stay visible in
              context, without exposing local paths.
            </p>
          </article>

          <article className="feature-card feature-branches">
            <div className="feature-number">05</div>
            <div className="branch-visual" aria-hidden="true">
              <span className="branch-root" />
              <span className="branch-line branch-a" />
              <span className="branch-line branch-b" />
              <div className="branch-node node-a">
                <i /> Original answer
              </div>
              <div className="branch-node node-b">
                <i /> Refined direction
              </div>
            </div>
            <p className="feature-tag">DURABLE CONVERSATIONS</p>
            <h3>Explore another path. Lose nothing.</h3>
            <p>
              Edit earlier prompts, regenerate responses, and switch between preserved conversation branches. Search,
              archive, export, and return after restart.
            </p>
          </article>

          <article className="feature-card feature-recovery">
            <div className="feature-number">06</div>
            <div className="recovery-visual" aria-hidden="true">
              <span className="recovery-ring">
                <b>✓</b>
              </span>
              <div>
                <strong>Everything saved</strong>
                <small>Response checkpointed · backup healthy</small>
              </div>
            </div>
            <p className="feature-tag">BUILT TO RECOVER</p>
            <h3>Your work survives the unexpected.</h3>
            <p>
              Partial responses are checkpointed as they arrive. Bottie detects interrupted work, checks its local
              store, and supports native backup and guided recovery.
            </p>
          </article>
        </div>
      </section>

      <section className="trust-section">
        <div className="trust-copy">
          <p className="kicker">A boundary you can see</p>
          <h2>
            The intelligence can change.
            <br />
            <em>The trust model doesn’t.</em>
          </h2>
          <p>
            Bottie’s Rust core owns secrets, files, memory, provider traffic, and tool policy. The interface receives
            narrow, typed information—never credentials or raw local paths.
          </p>
          <ul>
            <li>
              <span>✓</span> Secrets live in the operating system credential vault
            </li>
            <li>
              <span>✓</span> Remote routes are explicit before anything is sent
            </li>
            <li>
              <span>✓</span> Tool activity and supplied context stay inspectable
            </li>
          </ul>
        </div>
        <div className="boundary-diagram" aria-label="Bottie trust boundary diagram">
          <div className="boundary-box device-box">
            <small>YOUR DEVICE</small>
            <div className="boundary-item">
              <span className="boundary-icon">⌁</span>
              <div>
                <strong>Your context</strong>
                <small>Conversations · files · memory</small>
              </div>
            </div>
            <div className="boundary-core">
              <i className="brand-core" />
              <div>
                <strong>Bottie’s Rust core</strong>
                <small>Policy · storage · credentials</small>
              </div>
              <span>TRUST BOUNDARY</span>
            </div>
          </div>
          <div className="boundary-flow">
            <span>Only approved context</span>
            <i>→</i>
          </div>
          <div className="boundary-box model-box">
            <small>YOUR CHOICE</small>
            <div className="model-choice">
              <i className="provider-dot ollama" />
              <div>
                <strong>Local model</strong>
                <small>Stays on this device</small>
              </div>
            </div>
            <div className="or-divider">OR</div>
            <div className="model-choice">
              <i className="provider-dot openai" />
              <div>
                <strong>Cloud model</strong>
                <small>Explicit remote route</small>
              </div>
            </div>
          </div>
        </div>
      </section>

      <section className="section roadmap-section" id="roadmap">
        <div className="section-heading roadmap-heading">
          <div>
            <p className="kicker">Where Bottie is going</p>
            <h2>
              Built for today.
              <br />
              <em>Thinking beyond chat.</em>
            </h2>
          </div>
          <p>Development is moving in complete, testable slices toward a dependable cross-platform desktop release.</p>
        </div>

        <div className="roadmap-grid">
          <article className="roadmap-card now-card">
            <div className="roadmap-status">
              <span /> NOW · DEVELOPER PREVIEW
            </div>
            <h3>A private, connected AI workspace</h3>
            <p>
              Real local and cloud inference, durable conversations, files, memory, web research, email reading through
              the separately installed Localmail app, citations, and recovery foundations.
            </p>
            <div className="roadmap-tags">
              <span>Local + cloud</span>
              <span>Memory</span>
              <span>Web</span>
              <span>Email</span>
              <span>Files</span>
            </div>
          </article>
          <article className="roadmap-card next-card">
            <div className="roadmap-status">
              <span /> NEXT · DESKTOP BETA
            </div>
            <h3>Ready for sustained daily use</h3>
            <p>
              Security review, keyboard workflows, themes, accessibility, long-history performance, signed builds,
              updates, and packaging for macOS, Windows, and Linux.
            </p>
            <div className="roadmap-tags">
              <span>Polish</span>
              <span>Accessibility</span>
              <span>Packaging</span>
              <span>Updates</span>
            </div>
          </article>
          <article className="roadmap-card future-card">
            <div className="roadmap-status">
              <span /> LATER · LOCAL VOICE
            </div>
            <h3>Conversation that feels immediate</h3>
            <p>
              Private local speech-to-text and text-to-speech, voice activity detection, interruption, transcript
              correction, and full text fallback.
            </p>
            <div className="voice-wave" aria-hidden="true">
              <i />
              <i />
              <i />
              <i />
              <i />
              <i />
              <i />
              <i />
              <i />
            </div>
          </article>
        </div>
        <p className="roadmap-note">
          Roadmap items describe active direction, not release commitments. Bottie is currently a developer preview.
          Email capabilities require <a href={localmailUrl}>Localmail</a> to be installed and configured separately.
        </p>
      </section>

      <section className="closing-section">
        <div className="closing-core" aria-hidden="true">
          <span className="brand-core" />
        </div>
        <p className="kicker">A quieter kind of AI</p>
        <h2>
          Powerful enough to help.
          <br />
          <em>Transparent enough to trust.</em>
        </h2>
        <p>Follow Bottie’s progress as the developer preview grows into a polished desktop companion.</p>
        <a className="button button-primary" href={githubUrl} rel="noreferrer">
          Follow development on GitHub <span>↗</span>
        </a>
      </section>

      <footer>
        <a className="brand footer-brand" href="#top">
          <span className="brand-mark">
            <span className="brand-core" />
          </span>
          <span>bottie</span>
        </a>
        <p>Local-first. Open by design. Built with care.</p>
        <div>
          <a href="#capabilities">Capabilities</a>
          <a href="#roadmap">Roadmap</a>
          <a href={githubUrl} rel="noreferrer">
            GitHub ↗
          </a>
        </div>
      </footer>
    </main>
  );
}
