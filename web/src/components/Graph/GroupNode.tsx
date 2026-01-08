import { memo } from "react";
import type { GroupNodeData } from "./utils";
import "./GroupNode.css";

interface GroupNodeProps {
	data: GroupNodeData;
}

function GroupNode({ data }: GroupNodeProps) {
	const folderPath = data.folderPath || "/";
	const fileLabel = data.fileCount === 1 ? "file" : "files";

	return (
		<div className="group-node">
			<div className="group-node-label" title={folderPath}>
				<span className="group-node-path">{folderPath}</span>
				<span className="group-node-count">
					{data.fileCount} {fileLabel}
				</span>
			</div>
		</div>
	);
}

export default memo(GroupNode);
