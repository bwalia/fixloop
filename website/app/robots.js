export const dynamic = 'force-static';
export default function robots() {
  return { rules: { userAgent: '*', allow: '/' }, sitemap: 'https://fixloop.run/sitemap.xml' };
}
