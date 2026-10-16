# Jenga website

The buildless showcase and download page live in this directory. The Vercel project is `jenga` in `jeremys-projects-ae79fe79`; the current production alias is https://jenga-phi.vercel.app/. Intended canonical domain: https://jenga.sparkfield.com/.

Deploy from this directory using `vercel project inspect --non-interactive` to verify the target, then `vercel deploy --prod --scope jeremys-projects-ae79fe79`. Never commit `.vercel` or `.env*` files. GitHub Pages serves the same files from main `/docs`.

On 16 October 2026 Vercel accepted the custom domain, with ownership verified, but external DNS remained unconfigured. Atom nameservers were authoritative. Vercel requested a CNAME with name `jenga` and value `5942b016091890c7.vercel-dns-017.com.`. Re-run `vercel domains verify jenga.sparkfield.com --scope jeremys-projects-ae79fe79` after changing DNS. Preserve all unrelated records. Re-check the recommended value if configuration changes.

The site has three real project galleries. Maize Factory only has two schematic views. Hardware artifacts are demonstrations, not evidence of manufacturing readiness. Motion uses lightweight CSS, respects reduced-motion preferences and can be paused. `robots.txt` references the canonical sitemap.
