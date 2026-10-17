# Jenga website

The buildless showcase and download page live in this directory. The Vercel project is `jenga` in `jeremys-projects-ae79fe79`; the current production alias is https://jenga.sparkfield.co.ke/. Intended canonical domain: https://jenga.sparkfield.co.ke/.

Deploy from this directory using `vercel project inspect --non-interactive` to verify the target, then `vercel deploy --prod --scope jeremys-projects-ae79fe79`. Never commit `.vercel` or `.env*` files. GitHub Pages serves the same files from main `/docs`.

On 16 October 2026 the owner confirmed sparkfield.co.ke. The project is attached to jenga.sparkfield.co.ke; its existing Truehost DNS zone has the jenga CNAME pointing to 5942b016091890c7.vercel-dns-017.com. Existing nameservers and mail records were preserved. Verify DNS and HTTPS with the Vercel CLI after changes.

The site has three real project galleries, in order: 5 V / 555 pulse generator, Access Terminal S3, Maize Factory. The first two include actual self-hosted GLBs with camera controls and enlarged views. Maize Factory only has two schematic views. Hardware artifacts are demonstrations, not evidence of manufacturing readiness. Native schematic/PCB files and GLBs can be downloaded. Native boards may reference external project-specific libraries/models; GLBs are self-contained visual assemblies.

Orange dot motion uses CSS and respects reduced-motion preferences; there is no global pause button. Each model has its own rotation control. `robots.txt` references the five-page canonical sitemap, including individual project stories and image entries. The supplied Google URL-prefix HTML verification tag is retained in server-delivered page heads. This does not claim that Search Console verification or sitemap submission has been completed in Google's account UI.
