import { createContext, useContext, useEffect, useState } from "react";

export interface EditorContextProps {
  filePath?: string;
  projectRoot?: string;
  devServerPort?: number;
  devServerUrl?: string;
}

export const EditorContext = createContext<EditorContextProps>({});

// ============================================================================
// Global Store Hack for BlockNote Custom Blocks
// ============================================================================
// Due to BlockNote's rendering architecture, React Context updates might not
// propagate reliably to custom blocks deeper in the tree.
// We use a simple global store pattern to bypass this limitation.

let globalContext: EditorContextProps = {};
const listeners = new Set<(ctx: EditorContextProps) => void>();

export const updateGlobalContext = (ctx: EditorContextProps) => {
  globalContext = ctx;
  listeners.forEach((listener) => listener(ctx));
};

export const useGlobalEditorContext = () => {
  // Use React Context as initial value if available, or fall back to global store
  const reactContext = useContext(EditorContext);
  const [context, setContext] = useState<EditorContextProps>(
    Object.keys(reactContext).length > 0 ? reactContext : globalContext
  );

  useEffect(() => {
    // Sync with global store updates
    const listener = (ctx: EditorContextProps) => {
      setContext(ctx);
    };
    listeners.add(listener);

    // Also sync if global store is already newer (though rare in this pattern)
    if (globalContext !== context && Object.keys(globalContext).length > 0) {
      setContext(globalContext);
    }

    return () => {
      listeners.delete(listener);
    };
  }, []);

  return context;
};

// Keep original hook for standard usage
export const useEditorContext = () => useContext(EditorContext);
