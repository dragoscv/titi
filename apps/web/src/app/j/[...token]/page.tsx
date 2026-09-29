import { App } from "../../App";

// Invite links are /j/<uuid>/<key>/<exp>/<sig>/<name>: several segments, so a catch-all route.
export default async function JoinPage({ params }: { params: Promise<{ token: string[] }> }) {
  const { token } = await params;
  return <App joinLink={`titi://j/${token.join("/")}`} />;
}
