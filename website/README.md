# fixloop.run

The Fixloop landing page: a static Next.js export, published to GitHub Pages by
`.github/workflows/website.yml` on every push to `main` that touches `website/`.

```sh
cd website
npm ci
npm run dev      # http://localhost:3400
npm run build    # static site in out/
```

Design tokens are in `app/theme.css`, started from the TenthMatrix "Lattice"
theme (same fonts: Bricolage Grotesque, Instrument Sans, IBM Plex Mono). Keep
claims on the page in step with [ROADMAP.md](../ROADMAP.md): only say a stage is
available once it has shipped.

**Hosting:** `fixloop.run` and `www.fixloop.run` are Cloudflare DNS-only
CNAMEs to the WSL Proxy edge (`lon1.pop0.uk`), which terminates TLS and
proxies to GitHub Pages.
