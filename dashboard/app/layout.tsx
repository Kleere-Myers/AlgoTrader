import "./globals.css";
import Navbar from "@/components/Navbar";

export const metadata = {
  title: "AlgoTrader Dashboard",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body className="bg-navy-950 text-text-primary">
        <Navbar />
        <main className="w-full px-6 py-6">{children}</main>
      </body>
    </html>
  );
}
