import type { Metadata } from "next";
import { headers } from "next/headers";
import "./globals.css";

const title = "Bottie — Your context. Your models. Your rules.";
const description =
  "A private, local-first desktop AI companion with visible context, durable memory, and provider choice.";

export async function generateMetadata(): Promise<Metadata> {
  const requestHeaders = await headers();
  const host = requestHeaders.get("host") ?? "bottie.org";
  const protocol = requestHeaders.get("x-forwarded-proto") ?? (host.startsWith("localhost") ? "http" : "https");
  const baseUrl = new URL(`${protocol}://${host}`);
  const socialImage = new URL("/og.png", baseUrl).toString();

  return {
    metadataBase: baseUrl,
    title,
    description,
    alternates: { canonical: "/" },
    icons: { icon: "/favicon.png", shortcut: "/favicon.png" },
    openGraph: {
      type: "website",
      url: "/",
      siteName: "Bottie",
      title,
      description,
      images: [{ url: socialImage, width: 1731, height: 909, alt: title }],
    },
    twitter: { card: "summary_large_image", title, description, images: [socialImage] },
  };
}

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en">
      <head>
        <link rel="canonical" href="https://bottie.org/" />
      </head>
      <body>{children}</body>
    </html>
  );
}
