/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        ink: { DEFAULT: "#22344c", strong: "#0d1e35" },
        muted: "#617b98",
        surface: { DEFAULT: "#eef6ff", "2": "#e2efff" },
        card: "#ffffff",
        accent: { DEFAULT: "#1d7df2", "2": "#0abf7a", "3": "#12b9e8" },
        danger: "#e04545",
        ok: "#19a86b",
        warn: "#f59e0b",
      },
      borderRadius: {
        sm: "10px",
        md: "16px",
        lg: "22px",
        pill: "999px",
      },
      boxShadow: {
        glass: "0 20px 54px rgba(31, 73, 131, 0.12)",
        soft: "0 10px 24px rgba(31, 73, 131, 0.09)",
        btn: "0 10px 24px rgba(29, 125, 242, 0.32)",
        pill: "0 10px 20px rgba(21, 113, 239, 0.32)",
      },
      fontFamily: {
        sans: [
          '"HarmonyOS Sans SC"',
          '"Noto Sans SC"',
          '"PingFang SC"',
          '"Microsoft YaHei"',
          "system-ui",
          "sans-serif",
        ],
        mono: [
          '"JetBrains Mono"',
          '"SFMono-Regular"',
          '"Cascadia Code"',
          "Consolas",
          "monospace",
        ],
      },
    },
  },
  plugins: [],
};
