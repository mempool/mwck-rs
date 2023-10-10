let wallet;

const addressMap = {};
const addressList = [];
const regexAddress = /^([a-km-zA-HJ-NP-Z1-9]{26,35}|[a-km-zA-HJ-NP-Z1-9]{80}|[A-z]{2,5}1[a-zA-HJ-NP-Z0-9]{39,59}|04[a-fA-F0-9]{128}|(02|03)[a-fA-F0-9]{64})$/;
const addressInput = document.getElementById('addressInput');

function init_js(my_wallet) {
  wallet = my_wallet;

  wallet.subscribe();
  wallet.connect();

  document.getElementById('watchButton').addEventListener('click', onTrackAddress);
}
window.init_js = init_js;

export function onAddressEvent(address, tx_count, balance) {
  const table = document.getElementById('addressTable');
  const i = addressList.indexOf(address);
  const rows = table.querySelector('tbody').getElementsByTagName('tr');
  const row = rows[i];
  const cells = row.getElementsByTagName('td');
  if (balance) {
    const newBalance = (balance.mempool.funded + balance.confirmed.funded) - (balance.mempool.spent + balance.confirmed.spent);
    const prevBalance = Number(cells[1].textContent.slice(0, -4)) * 100_000_000;
    cells[1].textContent = `${(newBalance / 100_000_000).toFixed(8)} BTC`;
    cells[2].textContent = `${tx_count}`;
    if (prevBalance != null && !isNaN(prevBalance) && newBalance && !row.className) {
      if (prevBalance < newBalance) {
        row.classList.add("credit");
        row.classList.add("flash-once");
      } else if (prevBalance > newBalance) {
        row.classList.add("debit");
        row.classList.add("flash-once");
      } else {
        row.classList.add("conf");
        row.classList.add("flash-once");
      }
      let onAnimationEnd = () => {
        row.className = "";
        row.removeEventListener("animationend", onAnimationEnd);
      }
      row.addEventListener("animationend", onAnimationEnd);
    } 
  }
}

async function trackAddress(address) {
  if (regexAddress.test(address)) {
    if (!addressMap[address]) {
      addressMap[address] = true;
      addTableRow(address);
      addressList.push(address);
      return wallet.track_address(address);
    }
    return true;
  } else {
    alert(`${address} is not a valid bitcoin address!`);
    return false;
  }
}

async function onTrackAddress(event) {
  const address = addressInput.value
  if (await trackAddress(address)) {
    addressInput.value = '';
  }
}

function addTableRow(address) {
  const table = document.getElementById('addressTable');
  if (addressList.length === 0) {
    table.classList.remove('hidden');
  }
  const newRow = document.createElement('tr');
  const newCell = document.createElement('td');
  newCell.textContent = address;
  newRow.appendChild(newCell);
  for (let i = 0; i < 2; i++) {
    newRow.appendChild(document.createElement('td'));
  }
  table.querySelector('tbody').appendChild(newRow);
}