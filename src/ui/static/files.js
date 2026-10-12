/** Embedded UI adapter. All file operations use the public SDK; no private socket/codec. */
export const toViewModel = (value) =>
  Array.isArray(value)
    ? value.map(toViewModel)
    : value && typeof value === "object"
      ? Object.fromEntries(
          Object.entries(value).map(([key, item]) => [
            key.replace(/[A-Z]/g, (c) => "_" + c.toLowerCase()),
            toViewModel(item),
          ]),
        )
      : value;

export async function fileAction(client, action, payload) {
  const files = client.files;
  const ref = { rootId: payload.root_id, path: payload.relative_path };
  let result;
  switch (action) {
    case "ListDirectory":
      await files.goTo(payload.relative_path, { rootId: payload.root_id });
      result = await files.list({
        ...ref,
        cursor: payload.cursor,
        limit: payload.limit,
        includeHidden: payload.include_hidden,
        sort: { by: "name", direction: "asc" },
        filter: {
          name: payload.filter?.query ?? undefined,
          category: payload.filter?.filter ?? "All",
        },
      });
      break;
    case "ReadTextFile":
      result = await files.readText(ref);
      break;
    case "SaveTextFile":
      result = await files.writeText({
        ...ref,
        content: payload.content,
        expectedRevision: payload.expected_revision,
      });
      break;
    case "CreateFolder":
      result = await files.createFolder({
        rootId: payload.root_id,
        path: [payload.parent_relative_path, payload.folder_name]
          .filter(Boolean)
          .join("/"),
      });
      break;
    case "RenameFile":
      result = await files.rename({ ...ref, newName: payload.new_name });
      break;
    case "ChmodFile":
      result = await files.chmod({ ...ref, mode: payload.mode });
      break;
    case "MoveToTrash":
      result = await files.trash.move(ref);
      break;
    case "ListTrash":
      result = await files.trash.list({ rootId: payload.root_id });
      break;
    case "RestoreFromTrash":
      result = await files.trash.restore({ trashId: payload.trash_id });
      break;
    case "DeletePermanently":
      result = await files.trash.deletePermanently({
        rootId: payload.root_id,
        trashId: payload.relative_path.split("/").at(-1),
      });
      break;
    case "SearchFiles":
      result = await files.search({
        rootId: payload.root_id,
        path: payload.base_path,
        query: payload.query,
        limit: payload.max_results,
        includeHidden: payload.include_hidden,
        extension: payload.extension,
      });
      break;
    default:
      throw Error("Unsupported file action");
  }
  return toViewModel(result);
}
