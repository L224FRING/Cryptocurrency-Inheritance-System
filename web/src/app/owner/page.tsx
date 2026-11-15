import { OwnerDashboard } from "@/components/owner-dashboard";
import { PageHeader } from "@/components/page-header";

export default function OwnerPage() {
  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Owner"
        title="Owner console"
        description="Fund the vault, designate a beneficiary, check in while you are alive, and manage the FROST trustee committee."
      />
      <OwnerDashboard />
    </div>
  );
}
