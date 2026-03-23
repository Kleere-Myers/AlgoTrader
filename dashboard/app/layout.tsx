import "./globals.css";
import { DM_Sans, JetBrains_Mono } from "next/font/google";
import Navbar from "@/components/Navbar";

const dmSans = DM_Sans({
  subsets: ["latin"],
  variable: "--font-sans",
  display: "swap",
});

const jetbrainsMono = JetBrains_Mono({
  subsets: ["latin"],
  variable: "--font-mono",
  display: "swap",
});

export const metadata = {
  title: "AlgoTrader Dashboard",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" className={`${dmSans.variable} ${jetbrainsMono.variable}`}>
      <body className="bg-surface-950 text-text-primary font-sans antialiased">
        <Navbar />
        <main className="max-w-[1440px] mx-auto w-full px-6 py-6">{children}</main>
      </body>
    </html>
  );
}
