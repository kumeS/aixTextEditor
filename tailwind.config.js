/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      fontFamily: {
        // A calm, readable serif stack for the writing surface; sans for chrome.
        serif: ["Georgia", "Charter", "Cambria", "Times New Roman", "serif"],
        sans: [
          "-apple-system",
          "BlinkMacSystemFont",
          "Segoe UI",
          "Hiragino Kaku Gothic ProN",
          "Meiryo",
          "sans-serif",
        ],
      },
      colors: {
        ink: {
          DEFAULT: "#1f2933",
          soft: "#3e4c59",
          faint: "#7b8794",
        },
        accent: {
          DEFAULT: "#2563eb",
          soft: "#3b82f6",
        },
      },
      maxWidth: {
        prose: "44rem",
      },
    },
  },
  plugins: [],
};
