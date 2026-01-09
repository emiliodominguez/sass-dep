import { useContext } from "react";

import { ThemeContext } from "./themeContext";

/**
 * Hook for accessing theme context.
 * @returns Theme context value with theme state and controls
 * @throws Error if used outside of ThemeProvider
 */
export function useTheme() {
	const context = useContext(ThemeContext);

	if (!context) {
		throw new Error("useTheme must be used within a ThemeProvider");
	}

	return context;
}
