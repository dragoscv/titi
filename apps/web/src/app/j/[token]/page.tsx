import { App } from "../../App";

export default async function JoinPage({ params }: { params: Promise<{ token: string }> }) {
  const { token } = await params;
  return <App joinLink={`titi://j/${token}`} />;
}
