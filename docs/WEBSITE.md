# Jenga website

The buildless showcase and download page live in this directory. The Vercel project is `jenga` in `jeremys-projects-ae79fe79`; the current production alias is https://jenga.sparkfield.co.ke/. Intended canonical domain: https://jenga.sparkfield.co.ke/.

Deploy from this directory using `vercel project inspect --non-interactive` to verify the target, then `vercel deploy --prod --scope jeremys-projects-ae79fe79`. Never commit `.vercel` or `.env*` files. GitHub Pages serves the same files from main `/docs`.

On 16 October 2026 the owner confirmed sparkfield.co.ke. The project is attached to jenga.sparkfield.co.ke; its existing Truehost DNS zone has the jenga CNAME pointing to 5942b016091890c7.vercel-dns-017.com. Existing nameservers and mail records were preserved. Verify DNS and HTTPS with the Vercel CLI after changes.

The site has three real project galleries. Maize Factory only has two schematic views. Hardware artifacts are demonstrations, not evidence of manufacturing readiness. Motion uses lightweight CSS, respects reduced-motion preferences and can be paused. `robots.txt` references the canonical sitemap.
