import { Shell } from "@/components/Shell";

export default async function JoinPage({ params }: { params: Promise<{ token: string }> }) {
  const { token } = await params;
  return <Shell joinLink={`titi://j/${token}`} />;
}
