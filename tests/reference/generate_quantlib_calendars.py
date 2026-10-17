"""Regenerate calendar parity fixtures using pinned, unmodified QuantLib rules.

Requires Python 3 and a C++20 compiler (CXX, default clang++). Network access
is needed only for regeneration; Rust tests use the checked-in JSON fixture.
The small harness supplies Date/Easter utilities and compiles the original
isBusinessDay bodies, preserving C++ evaluation independently of the Rust port.
"""
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import urllib.request

COMMIT = "5f4811e7e03bbd1a3d6d7df31b751fecd5daef5f"
MARKETS = {
    "Australia": ["Settlement", "Asx"],
    "Canada": ["Settlement", "Tsx"],
    "Brazil": ["Settlement"],
    "Switzerland": [""],
    "NewZealand": ["Common", "Wellington", "Auckland"],
    "Norway": [""], "Sweden": [""], "Denmark": [""],
    "SouthKorea": ["Settlement", "Krx"],
    "SouthAfrica": [""], "China": ["Sse", "Ib"],
}
PRELUDE = r"""
#include <chrono>
#include <cstdint>
#include <iostream>
#include <set>
using Day = int; using Year = int;
enum Month {January=1, Jan=1, February=2, Feb=2, March=3, Mar=3,
 April=4, Apr=4, May=5, June=6, Jun=6, July=7, Jul=7, August=8, Aug=8,
 September=9, Sep=9, October=10, Oct=10, November=11, Nov=11, December=12, Dec=12};
enum Weekday {Sunday=1, Monday, Tuesday, Wednesday, Thursday, Friday, Saturday};
struct Date {
 std::chrono::sys_days value;
 Date(int d, Month m, int y): value(std::chrono::year(y)/unsigned(m)/unsigned(d)) {}
 Weekday weekday() const {return Weekday(std::chrono::weekday(value).c_encoding()+1);}
 int dayOfMonth() const {return unsigned(std::chrono::year_month_day(value).day());}
 Month month() const {return Month(unsigned(std::chrono::year_month_day(value).month()));}
 int year() const {return int(std::chrono::year_month_day(value).year());}
 int dayOfYear() const {return (value-std::chrono::sys_days(std::chrono::year(year())/1/1)).count()+1;}
 bool operator<(const Date& other) const {return value<other.value;}
};
bool isWeekend(Weekday w) {return w==Saturday || w==Sunday;}
int easterMonday(int y) {
 int a=y%19, b=y/100, c=y%100, d=b/4, e=b%4, f=(b+8)/25;
 int g=(b-f+1)/3, h=(19*a+b-d-g+15)%30, i=c/4, k=c%4;
 int l=(32+2*e+2*i-h-k)%7, m=(a+11*h+22*l)/451;
 int n=h+l-7*m+114;
 return Date(n%31+1, Month(n/31), y).dayOfYear()+1;
}
"""


def body(source, signature):
    start = source.index("{", source.index(signature))
    depth = 0
    for end in range(start, len(source)):
        depth += (source[end] == "{") - (source[end] == "}")
        if depth == 0:
            return source[start:end + 1]
    raise ValueError(signature)


def main():
    code = PRELUDE
    records = []
    calls = []
    for country, markets in MARKETS.items():
        url = f"https://raw.githubusercontent.com/lballabio/QuantLib/{COMMIT}/ql/time/calendars/{country.lower()}.cpp"
        with urllib.request.urlopen(url, timeout=30) as response:
            source = response.read().decode()
        for market in markets:
            name = f"{country}_{market or 'Default'}"
            original = body(source, f"bool {country}::{market}Impl::isBusinessDay")
            original = re.sub(r"static ext::shared_ptr<Calendar::Impl>.*?;", "", original, flags=re.S)
            original = original.replace("sseImpl->isBusinessDay(date)", "China_Sse(date)")
            original = original.replace("NewZealand::CommonImpl::isBusinessDay(date)", "NewZealand_Common(date)")
            original = original.replace("SettlementImpl::isBusinessDay(date)", "SouthKorea_Settlement(date)")
            code += f"\nbool {name}(const Date& date) {original}\n"
            if market == "Common":
                continue
            variant = {"Asx": "ASX", "Tsx": "TSX", "Krx": "KRX", "Sse": "SSE", "Ib": "IB"}.get(market, market)
            canonical = f"{country}({variant})" if market else country
            start, end = (2004, 2026) if country == "China" else (2004, 2050) if country == "SouthKorea" else (1901, 2052) if country == "NewZealand" else (1901, 2199)
            records.append({"calendar": canonical, "start_year": start, "end_year": end})
            calls.append(f"check({name}, {start}, {end});")
    code += r"""
void check(bool (*business)(const Date&), int first, int last) {
 std::uint64_t hash=14695981039346656037ULL;
 Date date(1,January,first), end(31,December,last);
 for (;date.value<=end.value;date.value+=std::chrono::days(1)) {
  hash^=business(date)?1:0; hash*=1099511628211ULL;
 }
 std::cout<<std::hex<<hash<<'\n';
}
int main() {
""" + "\n".join(calls) + "\n}\n"
    with tempfile.TemporaryDirectory(prefix="quantsupport-quantlib-reference-") as directory:
        source = Path(directory) / "reference.cpp"
        binary = Path(directory) / "reference"
        source.write_text(code)
        subprocess.run([os.environ.get("CXX", "clang++"), "-std=c++20", "-O2", str(source), "-o", str(binary)], check=True)
        hashes = subprocess.check_output([str(binary)], text=True).splitlines()
    assert len(hashes) == len(records)
    for record, digest in zip(records, hashes):
        record["business_day_hash"] = digest
    target = Path(__file__).resolve().parents[1] / "data/quantlib_calendars.json"
    target.write_text(json.dumps({"quantlib_commit": COMMIT, "hash": "FNV-1a 64 over daily business flags (0/1), inclusive dates", "calendars": records}, indent=2) + "\n")
    print(f"Generated {len(records)} calendar parity records: {target}")


if __name__ == "__main__":
    main()
