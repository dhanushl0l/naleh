function openModal(product) {
    productName.textContent = product.firstname;
    productPrice.innerHTML = `₹ ${product.price}`;
    mainImage.src = product.images[0];

    imageGallery.innerHTML = "";
    product.images.forEach(img => {
        const imgElem = document.createElement("img");
        imgElem.src = img;
        imgElem.onclick = () => mainImage.src = img;
        imageGallery.appendChild(imgElem);
    });

    modal.style.display = "flex";

    discription.querySelectorAll("p").forEach(p => p.remove());

    const discreption_p = document.createElement("p");
    discreption_p.textContent = `${product.description}`;
    discription.appendChild(discreption_p);

    info_box.querySelectorAll("p").forEach(p => p.remove());

    const info_box_weight = document.createElement("p");
    const info_box_type = document.createElement("p");

    info_box_weight.textContent = `Weight: ${product.weight}`;
    info_box_type.innerHTML = product.details;

    info_box.appendChild(info_box_weight);
    info_box.appendChild(info_box_type);
    button.onclick = () => {
        const url = navigator.userAgent.match(/Mobi|Android/i)
            ? `https://wa.me/8300299101?${product.url}`
            : `https://web.whatsapp.com/send?phone=8300299101&${product.url}`;
        window.open(url, "_blank");
    };
}
