// Downstream PostFiat decisive governance adapter for XRPLF/rippled 3.1.3.
// This is not an upstream XRPLF test.

#include <test/csf/Sim.h>

#include <xrpl/beast/unit_test.h>
#include <xrpl/json/json_reader.h>
#include <xrpl/json/json_writer.h>

#include <chrono>
#include <cstdlib>
#include <fstream>
#include <iostream>
#include <iterator>
#include <map>
#include <set>
#include <stdexcept>
#include <string>
#include <vector>

namespace ripple {
namespace test {

class DecisiveGovernanceBenchmark_test : public beast::unit_test::suite
{
    using Peer = csf::Peer;
    using PeerGroup = csf::PeerGroup;

    static Json::Value
    readJson(char const* path)
    {
        std::ifstream stream(path);
        if (!stream)
            throw std::runtime_error("cannot open decisive scenario manifest");
        std::string text{
            std::istreambuf_iterator<char>{stream},
            std::istreambuf_iterator<char>{}};
        Json::Value value;
        Json::Reader reader;
        if (!reader.parse(text, value))
            throw std::runtime_error("cannot parse decisive scenario manifest");
        return value;
    }

    static std::vector<std::string>
    strings(Json::Value const& value)
    {
        std::vector<std::string> result;
        for (auto const& row : value)
            result.push_back(row.asString());
        return result;
    }

    static bool
    contains(Json::Value const& value, std::string const& target)
    {
        for (auto const& row : value)
            if (row.asString() == target)
                return true;
        return false;
    }

    static std::set<std::string>
    effectiveUnavailable(Json::Value const& scenario)
    {
        if (scenario["event_schedule"]["recover_unavailable"].asBool())
            return {};
        auto const unavailable = strings(scenario["unavailable"]);
        return {unavailable.begin(), unavailable.end()};
    }

    static Json::Value
    governanceDecision(Json::Value const& scenario)
    {
        auto const unavailable = effectiveUnavailable(scenario);
        Json::Value nodes{Json::objectValue};
        std::set<std::string> decidedRoots;
        bool matchesOracle = true;
        for (auto const& nodeValue : scenario["correct_nodes"])
        {
            auto const node = nodeValue.asString();
            std::string outcome;
            std::string root;
            std::string reason;
            if (unavailable.count(node))
            {
                outcome = "unavailable";
                reason = "node remains unavailable at the observation boundary";
            }
            else
            {
                std::vector<std::string> admitted;
                auto const quorum = scenario["local_quorums"][node].asUInt();
                for (auto const& proposal : scenario["proposals"])
                {
                    std::set<std::string> supporters;
                    for (auto const& supporter : proposal["supporters"])
                        supporters.insert(supporter.asString());
                    std::size_t support = 0;
                    for (auto const& trusted : scenario["local_unls"][node])
                        if (supporters.count(trusted.asString()))
                            ++support;
                    if (support >= quorum)
                        admitted.push_back(proposal["registry_root"].asString());
                }
                if (admitted.size() == 1)
                {
                    outcome = "decide";
                    root = admitted.front();
                    reason = "one candidate reaches the node's local UNL quorum";
                    decidedRoots.insert(root);
                }
                else if (admitted.empty())
                {
                    outcome = "halt";
                    reason = "no candidate reaches the node's local UNL quorum";
                }
                else
                {
                    outcome = "halt";
                    reason = "multiple candidates reach local quorum; admission is ambiguous";
                }
            }

            auto const& expected = scenario["expected"]["rippled_nodes"][node];
            bool const rootMatches = expected.isMember("registry_root")
                ? root == expected["registry_root"].asString()
                : root.empty();
            bool const nodeMatches =
                outcome == expected["outcome"].asString() && rootMatches;
            matchesOracle = matchesOracle && nodeMatches;

            Json::Value row{Json::objectValue};
            row["outcome"] = outcome;
            if (!root.empty())
                row["registry_root"] = root;
            row["reason"] = reason;
            row["matches_oracle"] = nodeMatches;
            nodes[node] = std::move(row);
        }
        auto const conflicts = decidedRoots.empty() ? 0 : decidedRoots.size() - 1;
        bool const conflictMatches = conflicts ==
            scenario["expected"]["rippled_conflicting_roots"].asUInt();

        Json::Value result{Json::objectValue};
        result["model"] = "rippled-local-unl-governance-admission-v1";
        result["nodes"] = std::move(nodes);
        result["conflicting_roots"] =
            static_cast<Json::Value::UInt>(conflicts);
        result["expected_conflicting_roots"] =
            scenario["expected"]["rippled_conflicting_roots"];
        result["expectation_passed"] = matchesOracle && conflictMatches;
        return result;
    }

    static PeerGroup
    groupFrom(std::vector<Peer*> const& peers, std::set<std::size_t> const& indices)
    {
        std::vector<Peer*> selected;
        for (auto index : indices)
            selected.push_back(peers[index]);
        return PeerGroup{selected};
    }

    static Json::Value
    nativeLedgerControl(Json::Value const& scenario)
    {
        using namespace std::chrono;
        csf::Sim sim;
        auto const validators = strings(scenario["validators"]);
        auto all = sim.createGroup(validators.size());
        std::vector<Peer*> peers{all.begin(), all.end()};
        std::map<std::string, std::size_t> index;
        for (std::size_t i = 0; i < validators.size(); ++i)
            index.emplace(validators[i], i);

        auto const unavailableNames = effectiveUnavailable(scenario);
        std::set<std::size_t> unavailable;
        for (auto const& node : unavailableNames)
            unavailable.insert(index.at(node));

        for (std::size_t i = 0; i < validators.size(); ++i)
        {
            auto const& validator = validators[i];
            for (auto const& trusted : scenario["local_unls"][validator])
                peers[i]->trust(*peers[index.at(trusted.asString())]);
            peers[i]->quorum = scenario["local_quorums"][validator].asUInt();
            if (unavailable.count(i))
                peers[i]->runAsValidator = false;
        }
        for (std::size_t left = 0; left < peers.size(); ++left)
            for (std::size_t right = left + 1; right < peers.size(); ++right)
                if (!unavailable.count(left) && !unavailable.count(right))
                    peers[left]->connect(*peers[right], milliseconds{1});
        for (std::size_t i = 0; i < peers.size(); ++i)
            if (!unavailable.count(i))
                peers[i]->submit(csf::Tx{static_cast<std::uint32_t>(i + 1)});
        sim.run(seconds{20});

        std::set<std::size_t> observed;
        Json::Value nodes{Json::arrayValue};
        for (auto const& nodeValue : scenario["correct_nodes"])
        {
            auto const node = nodeValue.asString();
            auto const i = index.at(node);
            if (unavailable.count(i))
                continue;
            observed.insert(i);
            Json::Value row{Json::objectValue};
            row["validator"] = node;
            row["fully_validated_sequence"] = static_cast<std::uint32_t>(
                peers[i]->fullyValidatedLedger.seq());
            row["last_closed_sequence"] = static_cast<std::uint32_t>(
                peers[i]->lastClosedLedger.seq());
            row["completed_ledgers"] = peers[i]->completedLedgers;
            nodes.append(std::move(row));
        }
        auto const group = groupFrom(peers, observed);
        auto const branches = observed.empty() ? 0 : sim.branches(group);
        bool const synchronized = observed.empty() || sim.synchronized(group);

        Json::Value result{Json::objectValue};
        result["model"] = "rippled-3.1.3-native-csf-ledger-consensus";
        result["decision_scope"] = "ledger consensus control, not validator-governance admission";
        result["nodes"] = std::move(nodes);
        result["branches"] = static_cast<Json::Value::UInt>(branches);
        result["synchronized"] = synchronized;
        result["event_schedule_note"] =
            "governance schedule booleans are evaluated by the governance adapter; this native CSF control uses a deterministic connected transport";
        return result;
    }

    static Json::Value
    runScenario(Json::Value const& scenario)
    {
        auto governance = governanceDecision(scenario);
        auto ledger = nativeLedgerControl(scenario);
        Json::Value result{Json::objectValue};
        result["schema"] = "postfiat-rippled-decisive-case-v1";
        result["case_id"] = scenario["id"];
        result["fault_class"] = scenario["fault_class"];
        result["classification"] = scenario["expected"]["classification"];
        result["material_safety_delta"] =
            scenario["expected"]["material_safety_delta"];
        result["validator_governance"] = std::move(governance);
        result["native_ledger_consensus"] = std::move(ledger);
        result["expectation_passed"] =
            result["validator_governance"]["expectation_passed"];
        return result;
    }

public:
    void
    run() override
    {
        auto const* manifestPath =
            std::getenv("POSTFIAT_DECISIVE_SCENARIO_MANIFEST");
        auto const* outputPath =
            std::getenv("POSTFIAT_RIPPLED_DECISIVE_OUTPUT");
        if (!BEAST_EXPECT(manifestPath != nullptr && outputPath != nullptr))
            return;
        auto const manifest = readJson(manifestPath);
        if (!BEAST_EXPECT(
                manifest["schema"].asString() ==
                "postfiat-cobalt-decisive-manifest-v1"))
            return;

        testcase("decisive validator-governance manifest");
        Json::Value results{Json::arrayValue};
        std::size_t passed = 0;
        std::size_t conflicts = 0;
        for (auto const& scenario : manifest["cases"])
        {
            auto result = runScenario(scenario);
            if (result["expectation_passed"].asBool())
                ++passed;
            conflicts += result["validator_governance"]
                ["conflicting_roots"]
                    .asUInt();
            std::cout << "RIPPLED_DECISIVE_CASE "
                      << result["case_id"].asString() << " conflicts="
                      << result["validator_governance"]
                             ["conflicting_roots"]
                                 .asUInt()
                      << " pass=" << result["expectation_passed"].asBool()
                      << std::endl;
            results.append(std::move(result));
        }

        Json::Value report{Json::objectValue};
        report["schema"] = "postfiat-rippled-decisive-benchmark-report-v1";
        report["rippled_commit"] =
            "46b241ace8b30d9c9775d60ffba7d24b21903896";
        report["comparison_scope"] =
            "validator-governance decision; native CSF ledger consensus separately labeled";
        report["oracle_called"] = false;
        report["scenario_manifest_sha256"] = manifest["manifest_sha256"];
        report["case_count"] =
            static_cast<Json::Value::UInt>(results.size());
        report["passed_case_count"] =
            static_cast<Json::Value::UInt>(passed);
        report["conflicting_root_count"] =
            static_cast<Json::Value::UInt>(conflicts);
        report["results"] = std::move(results);
        report["status"] = passed == manifest["cases"].size()
            ? "passed"
            : "failed";
        std::ofstream output(outputPath);
        output << Json::StyledWriter{}.write(report);
        output.close();
        std::cout << "RIPPLED_DECISIVE_BENCHMARK cases="
                  << manifest["cases"].size() << " passed=" << passed
                  << " conflicts=" << conflicts << " status="
                  << report["status"].asString() << std::endl;
        BEAST_EXPECT(passed == manifest["cases"].size());
    }
};

BEAST_DEFINE_TESTSUITE(DecisiveGovernanceBenchmark, consensus, ripple);

}  // namespace test
}  // namespace ripple
