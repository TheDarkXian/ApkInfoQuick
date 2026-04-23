import React from "react";
import ReactDOM from "react-dom/client";
import { CssBaseline, ThemeProvider, createTheme } from "@mui/material";
import App from "./App";
import "./styles.css";

const theme = createTheme({
  palette: {
    mode: "light",
    primary: {
      main: "#0b57d0"
    },
    secondary: {
      main: "#137333"
    },
    background: {
      default: "#f8faff"
    }
  },
  shape: {
    borderRadius: 12
  },
  typography: {
    fontFamily: '"Noto Sans SC", "Segoe UI", "Helvetica Neue", Arial, sans-serif'
  }
});

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ThemeProvider theme={theme}>
      <CssBaseline />
      <App />
    </ThemeProvider>
  </React.StrictMode>
);
