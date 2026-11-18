import type { Metadata } from "next";
import { Cormorant_Garamond, IBM_Plex_Mono, Source_Serif_4 } from "next/font/google";
import "./globals.css";

/* ── Type ── */
const cormorant = Cormorant_Garamond({
  subsets: ["latin"],
  weight: ["500", "600", "700"],
  style: ["normal", "italic"],
  display: "swap",
  variable: "--font-cormorant",
});

const sourceSerif = Source_Serif_4({
  subsets: ["latin"],
  display: "swap",
  variable: "--font-source-serif",
});

const plexMono = IBM_Plex_Mono({
  subsets: ["latin"],
  weight: ["400", "500"],
  display: "swap",
  variable: "--font-plex-mono",
});

/* ── Meta ── */
export const metadata: Metadata = {
  metadataBase: new URL("https://galdr.app"),
  title: "galdr",
  description:
    "A desktop GUI wrapper around FFmpeg for converting and manipulating video, audio, and image files.",
  openGraph: {
    title: "galdr",
    description:
      "A desktop GUI wrapper around FFmpeg for converting and manipulating media files.",
    url: "https://galdr.app",
    siteName: "galdr",
  },
  twitter: {
    card: "summary",
    title: "galdr",
    description:
      "A desktop GUI wrapper around FFmpeg for converting and manipulating media files.",
  },
  robots: {
    index: true,
    follow: true,
  },
};

/* ── Structured data ── */
const jsonLd = {
  "@context": "https://schema.org",
  "@type": "SoftwareApplication",
  name: "galdr",
  operatingSystem: "Windows, macOS, Linux",
  applicationCategory: "Multimedia",
  description:
    "A desktop GUI wrapper around FFmpeg for converting and manipulating media files.",
  url: "https://galdr.app",
  downloadUrl: "https://github.com/aaen-studios/galdr/releases/latest",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en" className={`${cormorant.variable} ${sourceSerif.variable} ${plexMono.variable}`}>
      <head>
        <script
          type="application/ld+json"
          dangerouslySetInnerHTML={{ __html: JSON.stringify(jsonLd) }}
        />
      </head>
      <body>{children}</body>
    </html>
  );
}
