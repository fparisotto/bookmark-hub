/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./src/**/*.{html,js,rs}"],
  theme: {
    extend: {},
  },
  plugins: [
    require("@tailwindcss/typography"),
    require("@tailwindcss/forms"),
    require("daisyui"),
  ],
};
