export const API =
  window.location.port === "1420"
    ? "http://localhost:3001/api"
    : `${window.location.origin}/api`;
