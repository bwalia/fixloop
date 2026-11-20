export const dynamic = 'force-static';
export default function sitemap() {
  return [
    { url: 'https://fixloop.run/', changeFrequency: 'weekly', priority: 1 },
    { url: 'https://fixloop.run/privacy', changeFrequency: 'yearly', priority: 0.3 },
  ];
}
