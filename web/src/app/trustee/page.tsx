import { PageHeader } from "@/components/page-header";
import { TrusteeDashboard } from "@/components/trustee-dashboard";

export default function TrusteePage() {
  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Trustee"
        title="Trustee console"
        description="Confirm owner inactivity with a VDF proof, collect a threshold signature from the committee, and release the vault to the beneficiary."
      />
      <TrusteeDashboard />
    </div>
  );
}
