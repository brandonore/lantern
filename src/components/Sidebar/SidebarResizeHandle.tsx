import { useSidebarResize } from "../../hooks/useSidebarResize";
import styles from "./Sidebar.module.css";

export function SidebarResizeHandle() {
  const { onPointerDown } = useSidebarResize();

  return <div className={styles.resizeHandle} onPointerDown={onPointerDown} />;
}
